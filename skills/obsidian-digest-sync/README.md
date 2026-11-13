# Obsidian Digest Sync Skill

自動化對話摘要生成與 Obsidian Vault 雙向知識庫同步技能。

## 功能概覽

- **訊息收集** - 從 Buzz 頻道獲取指定日期範圍的對話
- **LLM 萃取** - 使用 Wax 系統提示詞提取結構化摘要
- **格式化輸出** - 生成符合規範的 Markdown 文件 (YAML Frontmatter + 結構化內文)
- **Vault 寫入** - 自動寫入 Obsidian Vault 並更新 INDEX.md 總索引
- **定時排程** - 支援 cron 表達式的守護進程模式 (Phase B)
- **重試機制** - 指數退避重試 (30s/60s/120s)
- **優雅關閉** - SIGINT/SIGTERM 信號處理

## 安裝與配置

### 1. 安裝依賴

```bash
cd C:/Users/user/.buzz/skills/obsidian-digest-sync
npm install
```

### 2. 環境變數配置

建立 `.env` 檔案或在系統環境變數中設定：

| 變數名稱 | 說明 | 預設值 |
|----------|------|--------|
| `BUZZ_DIGEST_SYNC_CHANNEL` | 目標頻道 ID | `5ca019f6-1a27-592a-b015-4baf0eb33572` |
| `BUZZ_OBSIDIAN_VAULT_PATH` | Obsidian Vault 路徑 | `c:/私人/ai agent/buzz-obsidian` |
| `BUZZ_DIGEST_OUTPUT_DIR` | 摘要輸出子目錄 | `02-Conversation-Digest` |
| `BUZZ_SYNC_OBSIDIAN_CMD` | 同步命令 | `node PLANS/sync_obsidian/main.mjs` |
| `BUZZ_SYNC_WORKING_DIR` | 同步工作目錄 | `/c/Users/user/.buzz` |
| `CRON_SCHEDULE` | Cron 排程表達式 | `0 2 * * *` (每天 02:00) |

### 3. Cron 排程範例

```bash
# 每天凌晨 02:00 執行 (預設，台灣時區)
CRON_SCHEDULE="0 2 * * *"

# 每小時執行一次
CRON_SCHEDULE="0 * * * *"

# 每週一至週五 06:00 執行
CRON_SCHEDULE="0 6 * * 1-5"

# 測試模式：每分鐘執行
CRON_SCHEDULE="* * * * *"
```

> ⚠️ 時區固定為 `Asia/Taipei`，請勿在 cron 表達式中包含時區資訊。

## 使用方式

### CLI 模式

```bash
# 單次收割昨天的對話
npm run sync-once
# 或
node main.js --run-once

# 單次收割指定日期
node main.js --run-once --date 2026-11-01

# 批次收割日期範圍
node main.js --batch --start 2026-11-01 --end 2026-11-03

# 每日模式 (依序處理多天)
node main.js --daily --start 2026-11-01 --end 2026-11-03
```

### Daemon 模式 (Phase B)

```bash
# 正式環境啟動
npm run daemon
# 或
node main.js --daemon

# 測試模式 (每分鐘觸發一次)
CRON_SCHEDULE="* * * * *" npm run daemon
# 或
CRON_SCHEDULE="* * * * *" node main.js --daemon
```

#### Daemon 控制

啟動後，Daemon 會常駐記憶體等待排程觸發：

- **手動觸發**：在程式碼中呼叫 `daemon.runNow()`
- **停止**：按 `Ctrl+C` 或發送 `SIGTERM` 信號
- **查看下次執行**：`daemon.getNextRun()`

## 部署指南

### Systemd 服務 (Linux/macOS/WSL)

建立 `/etc/systemd/system/obsidian-digest-sync.service`：

```ini
[Unit]
Description=Obsidian Digest Sync Daemon
After=network.target

[Service]
Type=simple
User=your-username
WorkingDirectory=/home/your-username/.buzz/skills/obsidian-digest-sync
Environment=BUZZ_DIGEST_SYNC_CHANNEL=5ca019f6-1a27-592a-b015-4baf0eb33572
Environment=BUZZ_OBSIDIAN_VAULT_PATH=/home/your-username/obsidian-vault
Environment=BUZZ_DIGEST_OUTPUT_DIR=02-Conversation-Digest
Environment=BUZZ_SYNC_OBSIDIAN_CMD=node PLANS/sync_obsidian/main.mjs
Environment=BUZZ_SYNC_WORKING_DIR=/home/your-username/.buzz
Environment=CRON_SCHEDULE=0 2 * * *
ExecStart=/usr/bin/node main.js --daemon
Restart=on-failure
RestartSec=10
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
```

啟用與啟動：

```bash
sudo systemctl daemon-reload
sudo systemctl enable obsidian-digest-sync
sudo systemctl start obsidian-digest-sync
sudo systemctl status obsidian-digest-sync
```

### PM2 進程管理 (跨平台)

```bash
# 安裝 PM2
npm install -g pm2

# 建立生態系統配置
cat > ecosystem.config.js << 'EOF'
module.exports = {
  apps: [{
    name: 'obsidian-digest-sync',
    script: 'main.js',
    args: '--daemon',
    cwd: 'C:/Users/user/.buzz/skills/obsidian-digest-sync',
    env: {
      BUZZ_DIGEST_SYNC_CHANNEL: '5ca019f6-1a27-592a-b015-4baf0eb33572',
      BUZZ_OBSIDIAN_VAULT_PATH: 'c:/私人/ai agent/buzz-obsidian',
      BUZZ_DIGEST_OUTPUT_DIR: '02-Conversation-Digest',
      BUZZ_SYNC_OBSIDIAN_CMD: 'node PLANS/sync_obsidian/main.mjs',
      BUZZ_SYNC_WORKING_DIR: '/c/Users/user/.buzz',
      CRON_SCHEDULE: '0 2 * * *'
    },
    error_file: './logs/err.log',
    out_file: './logs/out.log',
    log_file: './logs/combined.log',
    time: true,
    autorestart: true,
    max_restarts: 10,
    min_uptime: '30s'
  }]
};
EOF
```

## 檔案變更記錄 (Change Log)

### 2026-11-05 修復實作硬傷與環境強化 (Phase A/B 強化)

| 檔案 | 變更類型 | 說明 |
|------|----------|------|
| `digest_collector.mjs` | 修改 (fix) | 實作 `_resolveCredentials()` 自動載入 Relay 認證（`--private-key`, `--auth-tag`），新增 `_normalizePath` 解決 Windows 下 Git-bash POSIX 路徑引發之 `ENOENT` 錯誤。 |
| `digest_extractor.mjs` | 修改 (feat) | 移除寫死之 Mock 假資料，整合原生 `fetch` 調用 OpenRouter / OpenAI 相容 API 萃取器，並提供智慧啟發式降級萃取器 (`heuristicExtract`) 確保斷網或額度耗盡時不崩潰。 |
| `main.js` | 修改 (fix) | 修正 Windows 反斜線路徑導致 `import.meta.url` 比對失敗之 CLI 入口 Bug，修正預設工作目錄為 Windows 本地路徑。 |
| `digest_sync.mjs` | 修改 (fix) | 調整跨平台 `working_dir` 預設值，確保執行 `sync_obsidian` 雙向同步指令不拋出路徑異常。 |

