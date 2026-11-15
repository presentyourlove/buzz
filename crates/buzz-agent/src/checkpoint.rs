//! 階段 E：跨進程持久化加密 Checkpoint 模組（AES-256-GCM 認證加密、PII/Secret 去敏、路徑注入防護、72h TTL 自動清理）。
//!
//! 當 `buzz-acp` 因閒置休眠或管線錯誤重啟 `buzz-agent` 時，記憶體 `Session` 會歸零。
//! 本模組依據 `<context>` 中的 `channel_id` 與 `Thread root` 計算跨進程一致的 `scope_key`，
//! 並在落盤前強制對機敏資料去敏、以 AES-256-GCM 認證加密及原子寫入磁碟，確保安全續作且符合資料生命週期規範。

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::agent::{extract_reply_target_from_prompt, is_valid_uuid};
use crate::compaction::{
    extract_previous_handoff_summary, extract_verbatim_tail_for_handoff,
    find_latest_real_user_prompt, WorkingSetLedger,
};
use crate::config::{Config, DEFAULT_CHECKPOINT_MAX_FILES};
use crate::types::{HistoryItem, ToolResultContent};

/// 加密 Checkpoint 檔案魔術標頭與格式版本（BZCP01 = Buzz Checkpoint v1）。
const CHECKPOINT_MAGIC: &[u8; 6] = b"BZCP01";
/// 序列化酬載結構版本號，用於未來向後相容驗證。
const CHECKPOINT_PAYLOAD_VERSION: u32 = 1;
/// AES-256-GCM 標準 96-bit Nonce 長度（12 bytes）。
const CHECKPOINT_NONCE_LEN: usize = 12;
/// AES-256 金鑰長度（32 bytes）。
const CHECKPOINT_KEY_LEN: usize = 32;
/// `scope_key` 必須為固定 64 字元小寫 hex（BLAKE3 256-bit 摘要），兼作 Path Injection 白名單防線。
const SCOPE_KEY_HEX_LEN: usize = 64;
/// Checkpoint 正式存檔副檔名。
const CHECKPOINT_FILE_EXT: &str = "chk";
/// 預設頂層訊息討論串錨點字串（當 `<context>` 未指定 `Thread root` 或 `--reply-to` 時使用）。
const DEFAULT_THREAD_ANCHOR: &str = "channel_root";
/// 金鑰衍生領域分離鹽值。
const KEY_DERIVATION_DOMAIN: &[u8] = b"buzz-agent-aes256gcm-checkpoint-key-v1";
/// Scope Key 雜湊領域分離鹽值。
const SCOPE_KEY_DOMAIN: &[u8] = b"buzz-agent-checkpoint-v1:";
/// 機敏值遮罩標記。
const REDACTED_MARKER: &str = "[REDACTED]";
/// 允許被識別為機敏值（如 api_key=...）的最小 token 長度。
const MIN_SECRET_VALUE_LEN: usize = 8;
/// `sk-` 金鑰前綴後最少字元數。
const MIN_SK_BODY_LEN: usize = 20;
/// Nostr `nsec1` 私鑰固定主體長度（bech32 58 字元）。
const NSEC_BODY_LEN: usize = 58;
/// `Bearer <token>` 最小 token 長度。
const MIN_BEARER_LEN: usize = 16;
/// 單一系統暫存 Checkpoint 寫入鎖重試次數與間隔，避免剛切換進程時短暫競爭寫鎖。
const LOCK_RETRY_ATTEMPTS: usize = 5;
const LOCK_RETRY_DELAY_MS: u64 = 15;
/// 存入磁碟之歷史項目最大估算總位元組門檻：若超過則只保留前次摘要 + 逐字尾端 + 最新用戶提問。
const MAX_CHECKPOINT_RAW_HISTORY_BYTES: usize = 64 * 1024;

/// 跨進程討論串範圍識別鍵，由 `(agent_system_prompt_hash, channel_id, thread_anchor)` 確定性推導。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckpointScope {
    pub channel_id: String,
    pub thread_anchor: String,
    pub scope_key: String,
}

impl CheckpointScope {
    /// 從首輪用戶提問的 `<context>` 與特工 `system_prompt` 計算跨進程 Checkpoint Scope Key。
    ///
    /// 若提問中不含合法 UUID 格式的 `channel_id`（例如合成單元測試 `"first"`、`"turn 1"`），
    /// 回傳 `None` 停用讀寫，確保單元測試彼此隔離且不受磁碟既有存檔幹擾。
    pub fn from_prompt_and_system(prompt_text: &str, system_prompt: &str) -> Option<Self> {
        let (channel_id, reply_to) = extract_reply_target_from_prompt(prompt_text)?;
        if !is_valid_uuid(&channel_id) {
            return None;
        }

        let thread_anchor = extract_thread_root(prompt_text)
            .or(reply_to)
            .unwrap_or_else(|| DEFAULT_THREAD_ANCHOR.to_owned());

        let sys_hash = blake3::hash(system_prompt.as_bytes());
        let mut hasher = blake3::Hasher::new();
        hasher.update(SCOPE_KEY_DOMAIN);
        hasher.update(sys_hash.as_bytes());
        hasher.update(b":");
        hasher.update(channel_id.as_bytes());
        hasher.update(b":");
        hasher.update(thread_anchor.as_bytes());
        let scope_key = hasher.finalize().to_hex().to_string();

        Some(Self {
            channel_id,
            thread_anchor,
            scope_key,
        })
    }
}

fn extract_thread_root(prompt_text: &str) -> Option<String> {
    for line in prompt_text.lines() {
        let trimmed = line.trim();
        let Some(rest) = trimmed
            .strip_prefix("Thread root:")
            .or_else(|| trimmed.strip_prefix("thread_root:"))
        else {
            continue;
        };
        let candidate = rest
            .trim()
            .trim_matches(|c: char| c == '`' || c == '"' || c == '\'' || c == ',');
        if candidate.len() == 64 && candidate.chars().all(|c| c.is_ascii_hexdigit()) {
            return Some(candidate.to_ascii_lowercase());
        }
        if !candidate.is_empty()
            && candidate.len() <= 64
            && candidate
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Some(candidate.to_owned());
        }
    }
    None
}

/// 落盤前對純文字內容執行機敏資料與 PII 去敏（符合 `corerules.md` 第 2、3 節）。
///
/// 採 UTF-8 字元安全掃描器（無須額外 regex 依賴，支援含繁體中文的多 Byte 混合文字），
/// 遮罩 `sk-...`、`nsec1...`、`Bearer <token>` 及常見 `api_key/secret_key/access_token/password` 賦值。
pub fn scrub_sensitive_text(input: &str) -> String {
    let step1 = mask_prefixed_tokens(input);
    let step2 = mask_bearer_tokens(&step1);
    mask_kv_secrets(&step2)
}

fn is_token_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '-'
}

fn is_bech32_char(c: char) -> bool {
    matches!(
        c,
        'a' | 'c'..='h' | 'j'..='n' | 'p'..='z' | '0' | '2'..='9'
    )
}

/// 遮罩 `sk-[A-Za-z0-9_-]{20,}` 與 `nsec1[bech32]{58}` 前綴憑證（Unicode 邊界安全）。
fn mask_prefixed_tokens(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut i = 0;
    while i < input.len() {
        let rest = &input[i..];
        let prev_ok = i == 0
            || !input[..i]
                .chars()
                .next_back()
                .is_some_and(char::is_alphanumeric);

        if prev_ok && rest.starts_with("sk-") {
            let body = &rest[3..];
            let body_len = body
                .char_indices()
                .take_while(|(_, c)| is_token_char(*c))
                .last()
                .map(|(idx, c)| idx + c.len_utf8())
                .unwrap_or(0);
            if body_len >= MIN_SK_BODY_LEN {
                out.push_str(REDACTED_MARKER);
                i += 3 + body_len;
                continue;
            }
        }

        if prev_ok && rest.starts_with("nsec1") {
            let body = &rest[5..];
            let body_len = body
                .char_indices()
                .take_while(|(_, c)| is_bech32_char(*c))
                .last()
                .map(|(idx, c)| idx + c.len_utf8())
                .unwrap_or(0);
            if body_len >= NSEC_BODY_LEN {
                out.push_str(REDACTED_MARKER);
                i += 5 + body_len;
                continue;
            }
        }

        if let Some(ch) = rest.chars().next() {
            out.push(ch);
            i += ch.len_utf8();
        } else {
            break;
        }
    }
    out
}

/// 遮罩 `Bearer <token>` 授權標頭值。
fn mask_bearer_tokens(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut i = 0;
    while i < input.len() {
        let rest = &input[i..];
        let is_bearer = rest
            .get(..7)
            .is_some_and(|p| p.eq_ignore_ascii_case("Bearer "));
        let prev_ok = i == 0
            || !input[..i]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_ascii_alphanumeric());

        if prev_ok && is_bearer {
            let after_prefix = &rest[7..];
            let tok_len = after_prefix
                .char_indices()
                .take_while(|(_, c)| {
                    c.is_ascii_alphanumeric() || matches!(*c, '.' | '_' | '-' | '+' | '/' | '=')
                })
                .last()
                .map(|(idx, c)| idx + c.len_utf8())
                .unwrap_or(0);
            if tok_len >= MIN_BEARER_LEN {
                out.push_str(&rest[..7]);
                out.push_str(REDACTED_MARKER);
                i += 7 + tok_len;
                continue;
            }
        }

        if let Some(ch) = rest.chars().next() {
            out.push(ch);
            i += ch.len_utf8();
        } else {
            break;
        }
    }
    out
}

/// 遮罩 `api_key=...`、`secret_key=...`、`access_token=...`、`password=...` 等明文憑證賦值。
fn mask_kv_secrets(input: &str) -> String {
    const SECRET_KEYS: &[&str] = &[
        "api_key",
        "api-key",
        "apikey",
        "secret_key",
        "secret-key",
        "access_token",
        "access-token",
        "password",
        "passwd",
    ];
    let mut out = String::with_capacity(input.len());
    let mut i = 0;
    while i < input.len() {
        let rest = &input[i..];
        let prev_ok = i == 0
            || !input[..i]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_');

        let mut matched_secret = None;
        if prev_ok {
            for &key in SECRET_KEYS {
                let klen = key.len();
                if rest
                    .get(..klen)
                    .is_some_and(|prefix| prefix.eq_ignore_ascii_case(key))
                {
                    if let Some((prefix_len, val_len)) = parse_kv_assignment(&rest[klen..]) {
                        matched_secret = Some((klen + prefix_len, val_len));
                        break;
                    }
                }
            }
        }

        if let Some((keep_len, val_len)) = matched_secret {
            out.push_str(&rest[..keep_len]);
            out.push_str(REDACTED_MARKER);
            i += keep_len + val_len;
            continue;
        }

        if let Some(ch) = rest.chars().next() {
            out.push(ch);
            i += ch.len_utf8();
        } else {
            break;
        }
    }
    out
}

fn parse_kv_assignment(after_key: &str) -> Option<(usize, usize)> {
    let mut idx = 0;
    while idx < after_key.len() && after_key[idx..].starts_with(' ') {
        idx += 1;
    }
    if !after_key[idx..].starts_with(['=', ':']) {
        return None;
    }
    idx += 1;
    while idx < after_key.len() && after_key[idx..].starts_with(' ') {
        idx += 1;
    }
    if after_key[idx..].starts_with(['"', '\'']) {
        idx += 1;
    }
    let val_slice = &after_key[idx..];
    let val_len = val_slice
        .char_indices()
        .take_while(|(_, c)| !c.is_whitespace() && !matches!(*c, '"' | '\'' | ',' | ';' | ')'))
        .last()
        .map(|(end_i, c)| end_i + c.len_utf8())
        .unwrap_or(0);
    if val_len >= MIN_SECRET_VALUE_LEN && !val_slice[..val_len].starts_with(REDACTED_MARKER) {
        Some((idx, val_len))
    } else {
        None
    }
}

fn scrub_history_and_ledger(
    original_task: Option<String>,
    mut ledger: WorkingSetLedger,
    mut history: Vec<HistoryItem>,
) -> (Option<String>, WorkingSetLedger, Vec<HistoryItem>) {
    let scrubbed_task = original_task.map(|t| scrub_sensitive_text(&t));
    for f in &mut ledger.files {
        f.path = scrub_sensitive_text(&f.path);
    }
    for c in &mut ledger.commands {
        c.command_summary = scrub_sensitive_text(&c.command_summary);
    }
    ledger.last_error = ledger.last_error.take().map(|e| scrub_sensitive_text(&e));

    for item in &mut history {
        match item {
            HistoryItem::User(text) => *text = scrub_sensitive_text(text),
            HistoryItem::Assistant { text, .. } => {
                *text = scrub_sensitive_text(text);
            }
            HistoryItem::ToolResult(tr) => {
                for block in &mut tr.content {
                    if let ToolResultContent::Text(t) = block {
                        *t = scrub_sensitive_text(t);
                    }
                }
            }
        }
    }
    (scrubbed_task, ledger, history)
}

/// 當磁碟存檔前的記憶體 `history` 過於龐大時，精簡寫入 Checkpoint 的歷史切片，
/// 僅保留「既有摘要錨點 + 保護逐字尾端 + 最新用戶提問」，確保恢復快速且不爆新進程上下文。
pub fn compact_history_for_checkpoint(
    history: &[HistoryItem],
    original_task: Option<&str>,
    max_context_tokens: u64,
    tail_budget_pct: u64,
    tail_min_rounds: usize,
) -> Vec<HistoryItem> {
    let total_bytes: usize = history
        .iter()
        .map(HistoryItem::context_pressure_bytes)
        .sum();
    if total_bytes <= MAX_CHECKPOINT_RAW_HISTORY_BYTES {
        return history.to_vec();
    }

    let mut condensed = Vec::new();
    if let Some(prev) = extract_previous_handoff_summary(history) {
        condensed.push(HistoryItem::User(prev));
    }
    let latest_prompt = find_latest_real_user_prompt(history, original_task);
    let tail_budget =
        usize::try_from(max_context_tokens / 100 * tail_budget_pct).unwrap_or(usize::MAX);
    let tail = extract_verbatim_tail_for_handoff(
        history,
        tail_budget,
        tail_min_rounds,
        latest_prompt.as_deref(),
    );
    condensed.extend(tail);
    if let Some(p) = latest_prompt {
        condensed.push(HistoryItem::User(p));
    }
    condensed
}

/// 跨進程持久化 Checkpoint 酬載結構。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointPayload {
    pub version: u32,
    pub created_at_unix_secs: u64,
    pub updated_at_unix_secs: u64,
    pub channel_id: String,
    pub thread_anchor: String,
    pub original_task: Option<String>,
    pub handoff_count: usize,
    pub ledger: WorkingSetLedger,
    pub history: Vec<HistoryItem>,
}

/// AES-256-GCM 靜態加密 Checkpoint 儲存管理員（含路徑安全驗證、`fs2` 鎖定與 TTL 自動清理）。
#[derive(Debug, Clone)]
pub struct EncryptedCheckpointStore {
    dir: PathBuf,
    ttl_secs: u64,
    key: [u8; CHECKPOINT_KEY_LEN],
}

impl EncryptedCheckpointStore {
    /// 依據執行期 `Config` 與當前特工 `system_prompt` 建立加密儲存實例。
    pub fn from_config(cfg: &Config, system_prompt: &str) -> Self {
        let dir = resolve_checkpoint_dir(cfg);
        let key = derive_checkpoint_key(system_prompt, cfg.agent_keys.as_ref());
        Self {
            dir,
            ttl_secs: cfg.checkpoint_ttl_secs,
            key,
        }
    }

    /// 供單元測試與自訂環境明確指定目錄、TTL 與 256-bit 金鑰建構儲存管理員。
    pub fn with_explicit_key(dir: PathBuf, ttl_secs: u64, key: [u8; CHECKPOINT_KEY_LEN]) -> Self {
        Self { dir, ttl_secs, key }
    }

    pub fn checkpoint_dir(&self) -> &Path {
        &self.dir
    }

    /// 將討論串狀態進行機敏去敏、AES-256-GCM 加密，並透過 `fs2` 檔案鎖與 `.tmp` + `rename` 原子寫入磁碟。
    pub fn save(
        &self,
        scope: &CheckpointScope,
        original_task: Option<String>,
        handoff_count: usize,
        ledger: WorkingSetLedger,
        history: Vec<HistoryItem>,
    ) -> Result<(), String> {
        validate_scope_key(&scope.scope_key)?;
        fs::create_dir_all(&self.dir)
            .map_err(|e| format!("checkpoint: create dir {}: {e}", self.dir.display()))?;

        // 每次寫入順手執行資料生命週期清理（72h TTL 與單目錄上限 200 檔）
        let _ = self.purge_expired();

        let now = unix_now_secs();
        let (scrubbed_task, scrubbed_ledger, scrubbed_history) =
            scrub_history_and_ledger(original_task, ledger, history);

        let payload = CheckpointPayload {
            version: CHECKPOINT_PAYLOAD_VERSION,
            created_at_unix_secs: now,
            updated_at_unix_secs: now,
            channel_id: scope.channel_id.clone(),
            thread_anchor: scope.thread_anchor.clone(),
            original_task: scrubbed_task,
            handoff_count,
            ledger: scrubbed_ledger,
            history: scrubbed_history,
        };
        let plaintext = serde_json::to_vec(&payload)
            .map_err(|e| format!("checkpoint: serialize payload: {e}"))?;

        let mut nonce_bytes = [0u8; CHECKPOINT_NONCE_LEN];
        getrandom::fill(&mut nonce_bytes)
            .map_err(|e| format!("checkpoint: rng failed for nonce: {e}"))?;

        let cipher = Aes256Gcm::new_from_slice(&self.key)
            .map_err(|e| format!("checkpoint: init cipher: {e}"))?;
        let nonce = Nonce::from_slice(&nonce_bytes);
        let ciphertext = cipher
            .encrypt(nonce, plaintext.as_ref())
            .map_err(|_| "checkpoint: AEAD encryption failed".to_owned())?;

        let mut wire =
            Vec::with_capacity(CHECKPOINT_MAGIC.len() + CHECKPOINT_NONCE_LEN + ciphertext.len());
        wire.extend_from_slice(CHECKPOINT_MAGIC);
        wire.extend_from_slice(&nonce_bytes);
        wire.extend_from_slice(&ciphertext);

        let lock_path = self.safe_path(&format!("{}.lock", scope.scope_key))?;
        let chk_path = self.safe_path(&format!("{}.{}", scope.scope_key, CHECKPOINT_FILE_EXT))?;
        let tmp_path =
            self.safe_path(&format!("{}.tmp.{}", scope.scope_key, uuid::Uuid::new_v4()))?;

        let lock_file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(&lock_path)
            .map_err(|e| format!("checkpoint: open lock {}: {e}", lock_path.display()))?;

        acquire_lock_with_retry(&lock_file)?;

        let write_result = (|| -> Result<(), String> {
            let mut tmp = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&tmp_path)
                .map_err(|e| format!("checkpoint: create tmp {}: {e}", tmp_path.display()))?;
            tmp.write_all(&wire)
                .map_err(|e| format!("checkpoint: write tmp: {e}"))?;
            tmp.sync_all()
                .map_err(|e| format!("checkpoint: sync tmp: {e}"))?;
            // Windows 平台執行 rename 前必須關閉檔案控制代碼以免發生共享違規
            drop(tmp);
            fs::rename(&tmp_path, &chk_path)
                .map_err(|e| format!("checkpoint: rename to {}: {e}", chk_path.display()))?;
            Ok(())
        })();

        if write_result.is_err() {
            let _ = fs::remove_file(&tmp_path);
        }
        let _ = FileExt::unlock(&lock_file);
        write_result
    }

    /// 載入並解密指定 `scope_key` 的 Checkpoint；若檔案不存在或已過期則刪除並回傳 `Ok(None)`，
    /// 若密文遭篡改（AEAD Tag 驗證失敗）則回傳 `Err`。
    pub fn load(&self, scope_key: &str) -> Result<Option<CheckpointPayload>, String> {
        validate_scope_key(scope_key)?;
        let chk_path = self.safe_path(&format!("{scope_key}.{CHECKPOINT_FILE_EXT}"))?;
        if !chk_path.exists() {
            return Ok(None);
        }

        if let Ok(meta) = fs::metadata(&chk_path) {
            if is_metadata_expired(&meta, self.ttl_secs) {
                let _ = fs::remove_file(&chk_path);
                return Ok(None);
            }
        }

        let bytes = fs::read(&chk_path)
            .map_err(|e| format!("checkpoint: read {}: {e}", chk_path.display()))?;
        let min_len = CHECKPOINT_MAGIC.len() + CHECKPOINT_NONCE_LEN;
        if bytes.len() <= min_len || &bytes[..CHECKPOINT_MAGIC.len()] != CHECKPOINT_MAGIC {
            return Err("checkpoint: invalid magic header or truncated file".to_owned());
        }

        let nonce_start = CHECKPOINT_MAGIC.len();
        let ct_start = nonce_start + CHECKPOINT_NONCE_LEN;
        let nonce = Nonce::from_slice(&bytes[nonce_start..ct_start]);
        let ciphertext = &bytes[ct_start..];

        let cipher = Aes256Gcm::new_from_slice(&self.key)
            .map_err(|e| format!("checkpoint: init cipher: {e}"))?;
        let plaintext = cipher
            .decrypt(nonce, ciphertext)
            .map_err(|_| "checkpoint: AEAD authentication tag verification failed".to_owned())?;

        let payload: CheckpointPayload = serde_json::from_slice(&plaintext)
            .map_err(|e| format!("checkpoint: deserialize payload: {e}"))?;
        if payload.version != CHECKPOINT_PAYLOAD_VERSION {
            return Err(format!(
                "checkpoint: unsupported version {}",
                payload.version
            ));
        }

        let now = unix_now_secs();
        if payload.updated_at_unix_secs.saturating_add(self.ttl_secs) < now {
            let _ = fs::remove_file(&chk_path);
            return Ok(None);
        }

        Ok(Some(payload))
    }

    /// 清除所有已逾 `ttl_secs`（預設 72 小時）或超出單目錄檔案上限（200 檔）的 Checkpoint 存檔。
    pub fn purge_expired(&self) -> usize {
        let Ok(entries) = fs::read_dir(&self.dir) else {
            return 0;
        };
        let mut purged = 0usize;
        let mut active: Vec<(PathBuf, SystemTime)> = Vec::new();

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some(CHECKPOINT_FILE_EXT) {
                continue;
            }
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            if is_metadata_expired(&meta, self.ttl_secs) {
                if fs::remove_file(&path).is_ok() {
                    purged += 1;
                }
                continue;
            }
            let mtime = meta.modified().unwrap_or(UNIX_EPOCH);
            active.push((path, mtime));
        }

        if active.len() > DEFAULT_CHECKPOINT_MAX_FILES {
            active.sort_by_key(|(_, mtime)| *mtime);
            let excess = active.len() - DEFAULT_CHECKPOINT_MAX_FILES;
            for (old_path, _) in active.into_iter().take(excess) {
                if fs::remove_file(old_path).is_ok() {
                    purged += 1;
                }
            }
        }
        purged
    }

    /// 建立交易式微快照：在執行可能破壞環境或代碼的工具前，為指定檔案列表建立暫存快照。
    pub fn create_micro_snapshot(
        &self,
        scope_key: &str,
        files: &[PathBuf],
    ) -> Result<String, String> {
        validate_scope_key(scope_key)?;
        let ts = unix_now_secs();
        let snapshot_id = format!("{scope_key}_{ts}");
        let snapshot_dir = self.dir.join("snapshots").join(&snapshot_id);
        fs::create_dir_all(&snapshot_dir)
            .map_err(|e| format!("micro_snapshot: create dir {}: {e}", snapshot_dir.display()))?;

        let mut manifest: Vec<(String, String)> = Vec::new();

        for (idx, original) in files.iter().enumerate() {
            if original.exists() && original.is_file() {
                let backup_name = format!("file_{idx}.bak");
                let backup_path = snapshot_dir.join(&backup_name);
                fs::copy(original, &backup_path).map_err(|e| {
                    format!(
                        "micro_snapshot: backup {} -> {}: {e}",
                        original.display(),
                        backup_path.display()
                    )
                })?;
                manifest.push((original.to_string_lossy().to_string(), backup_name));
            }
        }

        let manifest_json = serde_json::to_string(&manifest)
            .map_err(|e| format!("micro_snapshot: serialize manifest: {e}"))?;
        fs::write(snapshot_dir.join("manifest.json"), manifest_json)
            .map_err(|e| format!("micro_snapshot: write manifest: {e}"))?;

        Ok(snapshot_id)
    }

    /// 回滾交易式微快照：當執行遭遇致命錯誤或自我修正失敗時，將原檔案原子還原。
    pub fn rollback_micro_snapshot(&self, snapshot_id: &str) -> Result<(), String> {
        let snapshot_dir = self.dir.join("snapshots").join(snapshot_id);
        if !snapshot_dir.exists() {
            return Err(format!("micro_snapshot: snapshot_id '{snapshot_id}' not found"));
        }

        let manifest_path = snapshot_dir.join("manifest.json");
        if !manifest_path.exists() {
            return Err("micro_snapshot: manifest.json missing".to_string());
        }

        let manifest_content = fs::read_to_string(&manifest_path)
            .map_err(|e| format!("micro_snapshot: read manifest: {e}"))?;
        let manifest: Vec<(String, String)> = serde_json::from_str(&manifest_content)
            .map_err(|e| format!("micro_snapshot: parse manifest: {e}"))?;

        for (orig_str, backup_name) in manifest {
            let orig_path = PathBuf::from(orig_str);
            let backup_path = snapshot_dir.join(backup_name);
            if backup_path.exists() {
                let _ = fs::copy(&backup_path, &orig_path);
            }
        }

        let _ = fs::remove_dir_all(&snapshot_dir);
        Ok(())
    }

    /// 正常完成後丟棄交易式微快照
    pub fn discard_micro_snapshot(&self, snapshot_id: &str) -> Result<(), String> {
        let snapshot_dir = self.dir.join("snapshots").join(snapshot_id);
        if snapshot_dir.exists() {
            let _ = fs::remove_dir_all(&snapshot_dir);
        }
        Ok(())
    }

    fn safe_path(&self, file_name: &str) -> Result<PathBuf, String> {
        if file_name.contains('/') || file_name.contains('\\') || file_name.contains("..") {
            return Err("checkpoint: illegal path component detected".to_owned());
        }
        Ok(self.dir.join(file_name))
    }
}

/// 驗證 `scope_key` 必須嚴格為 64 個小寫 ASCII Hex 字元（杜絕 Path Injection）。
pub fn validate_scope_key(scope_key: &str) -> Result<(), String> {
    if scope_key.len() != SCOPE_KEY_HEX_LEN
        || !scope_key
            .chars()
            .all(|c| matches!(c, '0'..='9' | 'a'..='f'))
    {
        return Err(
            "checkpoint: invalid scope_key format (must be 64 lowercase hex chars)".to_owned(),
        );
    }
    Ok(())
}

fn resolve_checkpoint_dir(cfg: &Config) -> PathBuf {
    if let Some(custom) = &cfg.checkpoint_dir {
        return custom.clone();
    }
    dirs::data_local_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("Buzz")
        .join("checkpoints")
}

/// 由本機環境與特工身分指紋衍生 256-bit AES-GCM 靜態加密金鑰，絕不在原始碼硬編碼金鑰。
fn derive_checkpoint_key(
    system_prompt: &str,
    agent_keys: Option<&Arc<nostr::Keys>>,
) -> [u8; CHECKPOINT_KEY_LEN] {
    let mut hasher = Sha256::new();
    hasher.update(KEY_DERIVATION_DOMAIN);

    if let Ok(env_key) = std::env::var("BUZZ_AGENT_CHECKPOINT_KEY") {
        let trimmed = env_key.trim();
        if !trimmed.is_empty() {
            hasher.update(b":env:");
            hasher.update(trimmed.as_bytes());
        }
    }
    if let Some(keys) = agent_keys {
        hasher.update(b":nostr:");
        hasher.update(keys.public_key().to_hex().as_bytes());
        hasher.update(b":");
        hasher.update(blake3::hash(keys.secret_key().to_secret_hex().as_bytes()).as_bytes());
    }
    if let Some(local_dir) = dirs::data_local_dir().or_else(dirs::home_dir) {
        hasher.update(b":local:");
        hasher.update(local_dir.to_string_lossy().as_bytes());
    }
    hasher.update(b":sys:");
    hasher.update(blake3::hash(system_prompt.as_bytes()).as_bytes());

    let digest = hasher.finalize();
    let mut key = [0u8; CHECKPOINT_KEY_LEN];
    key.copy_from_slice(&digest);
    key
}

fn acquire_lock_with_retry(file: &File) -> Result<(), String> {
    for attempt in 0..LOCK_RETRY_ATTEMPTS {
        if file.try_lock_exclusive().is_ok() {
            return Ok(());
        }
        if attempt + 1 < LOCK_RETRY_ATTEMPTS {
            std::thread::sleep(Duration::from_millis(LOCK_RETRY_DELAY_MS));
        }
    }
    Err("checkpoint: busy lock on scope file".to_owned())
}

fn unix_now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn is_metadata_expired(meta: &fs::Metadata, ttl_secs: u64) -> bool {
    let Ok(mtime) = meta.modified() else {
        return false;
    };
    let Ok(elapsed) = SystemTime::now().duration_since(mtime) else {
        return false;
    };
    elapsed.as_secs() > ttl_secs
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Map};
    use tempfile::tempdir;

    use super::*;
    use crate::compaction::LedgerFileEntry;
    use crate::types::{ToolCall, ToolResult};

    #[test]
    fn scope_key_matches_for_same_channel_thread_and_skips_non_uuid_tests() {
        assert!(CheckpointScope::from_prompt_and_system("turn 1", "sys").is_none());
        assert!(CheckpointScope::from_prompt_and_system("first", "sys").is_none());

        let thread_hex = "a".repeat(64);
        let p1 = format!(
            "<context>\nChannel: ops (#5ca019f6-1a27-592a-b015-4baf0eb33572)\nThread root: {thread_hex}\n</context>\nhello"
        );
        let p2 = format!(
            "<context>\nChannel: ops (#5ca019f6-1a27-592a-b015-4baf0eb33572)\nThread root: {thread_hex}\n</context>\ndifferent message later"
        );
        let s1 = CheckpointScope::from_prompt_and_system(&p1, "pollen-prompt").unwrap();
        let s2 = CheckpointScope::from_prompt_and_system(&p2, "pollen-prompt").unwrap();
        assert_eq!(s1.scope_key, s2.scope_key);
        assert_eq!(s1.scope_key.len(), 64);

        let other_thread = "b".repeat(64);
        let p3 = format!(
            "<context>\nChannel: ops (#5ca019f6-1a27-592a-b015-4baf0eb33572)\nThread root: {other_thread}\n</context>\nhi"
        );
        let s3 = CheckpointScope::from_prompt_and_system(&p3, "pollen-prompt").unwrap();
        assert_ne!(s1.scope_key, s3.scope_key);
    }

    #[test]
    fn scrub_sensitive_text_redacts_keys_bearer_nsec_and_passwords() {
        let sk_secret = "sk-ant-api03-abcdefghijklmnopqrstuvwxyz1234567890";
        let nsec_secret = "nsec1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqs2v9z7";
        let input = format!(
            "使用金鑰 {sk_secret} 與私鑰 {nsec_secret}，header=Bearer my_super_secret_token_98765，設定 api_key=\"topsecretvalue123\" 與 password=my_db_pass!"
        );
        let scrubbed = scrub_sensitive_text(&input);
        assert!(!scrubbed.contains(sk_secret));
        assert!(!scrubbed.contains(nsec_secret));
        assert!(!scrubbed.contains("my_super_secret_token_98765"));
        assert!(!scrubbed.contains("topsecretvalue123"));
        assert!(!scrubbed.contains("my_db_pass"));
        assert!(scrubbed.contains("Bearer [REDACTED]"));
        assert!(scrubbed.contains("繁體") || scrubbed.contains("使用金鑰 [REDACTED]"));
    }

    #[test]
    fn aes256gcm_store_encrypts_scrubs_detects_tamper_and_purges_expired() {
        let dir = tempdir().unwrap();
        let key = [0x42u8; 32];
        let store =
            EncryptedCheckpointStore::with_explicit_key(dir.path().to_path_buf(), 3600, key);

        let scope = CheckpointScope::from_prompt_and_system(
            "Channel: ops (#5ca019f6-1a27-592a-b015-4baf0eb33572)\n--reply-to 0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            "agent-sys",
        )
        .unwrap();

        let ledger = WorkingSetLedger {
            files: vec![LedgerFileEntry {
                path: "src/main.rs".into(),
                op: "edit".into(),
                status: "ok".into(),
            }],
            commands: vec![],
            last_error: Some("api_key=leak_secret_password_99".into()),
        };
        let history = vec![
            HistoryItem::User(
                "[Context Handoff]\nsummary with sk-proj-abcdefghijklmnopqrstuvwxyz12345".into(),
            ),
            HistoryItem::Assistant {
                text: "done".into(),
                tool_calls: vec![ToolCall {
                    provider_id: "c1".into(),
                    name: "dev__read_file".into(),
                    arguments: json!({"path": "src/main.rs"}),
                    provider_extra: Map::new(),
                }],
                reasoning_details: None,
            },
            HistoryItem::ToolResult(ToolResult {
                provider_id: "c1".into(),
                content: vec![ToolResultContent::Text("ok".into())],
                is_error: false,
            }),
        ];

        store
            .save(&scope, Some("original task".into()), 2, ledger, history)
            .expect("checkpoint save should succeed");

        let chk_path = dir.path().join(format!("{}.chk", scope.scope_key));
        let raw_cipher = fs::read(&chk_path).unwrap();
        assert_eq!(&raw_cipher[..6], b"BZCP01");
        let raw_lossy = String::from_utf8_lossy(&raw_cipher);
        assert!(!raw_lossy.contains("original task"));
        assert!(!raw_lossy.contains("sk-proj-abcdefghijklmnopqrstuvwxyz12345"));

        // 正常載入並確認機敏值已去敏
        let loaded = store.load(&scope.scope_key).unwrap().unwrap();
        assert_eq!(loaded.handoff_count, 2);
        assert_eq!(loaded.original_task.as_deref(), Some("original task"));
        assert!(!loaded
            .ledger
            .last_error
            .as_deref()
            .unwrap()
            .contains("leak_secret_password_99"));
        if let HistoryItem::User(u) = &loaded.history[0] {
            assert!(!u.contains("sk-proj-abcdefghijklmnopqrstuvwxyz12345"));
            assert!(u.contains("[REDACTED]"));
        }

        // 篡改密文最後 1 byte 必須遭 AEAD Tag 拒絕
        let mut tampered = raw_cipher.clone();
        let last = tampered.len() - 1;
        tampered[last] ^= 0xFF;
        fs::write(&chk_path, tampered).unwrap();
        assert!(store.load(&scope.scope_key).is_err());

        // TTL = 0 時 purge_expired（或手動過期測試）能清理舊檔；路徑注入驗證拒絕非法 key
        assert!(validate_scope_key("../evil").is_err());
        assert!(validate_scope_key("abc").is_err());
    }
}
