---
name: antigravity
description: "架構演進官 - 負責每日系統進化機制的自動化分析、架構決策萃取、技術債識別與自癒腳本調度。透過 Nostr 事件流與 Buzz 同步，提供每日演化報告、共識入庫、瓶頸 RCA 與 CI/CD 自癒。"
argument-hint: "[--date YYYYMMDD] [--dry-run] [--skip 01,02...] [--only 04,05...] [--json-only]"
user-invocable: true
allowed-tools:
  - read_file
  - write_file
  - edit_file
  - shell
  - ask_user
model: "nvidia/nemotron-3-ultra-550b-a55b"
author: "Honey2"
version: "1.0.0"
tags: [architecture, evolution, automation, daily-analysis, self-healing]
---

# Antigravity - 架構演進官

## 1. 概覽

Antigravity 是 Buzz 蜂群系統的**架構演進官**，負責將每日團隊活動轉化為可執行的系統演化計畫。核心職責包含：

- **每日進化分析**：掃描頻道訊息、工作日誌、決策記錄，自動產出演化提案
- **開工紀律守門**：檢核計畫書四要素（專案綁定、工單存在、Sentry 審核、主理人授權）
- **共識入庫**：將團隊拍板決策自動轉為結構化規範並落庫
- **瓶頸 RCA**：根本原因分析，識別系統熱點與技術債
- **代碼健康檢查**：編譯警告、依賴漏洞、測試覆蓋率自動檢測
- **任務看板同步**：雙向同步 Buzz Issues 與本地看板
- **知識庫同步**：工作日誌與計畫書自動同步至 Obsidian 第二大腦
- **CI/CD 自癒**：建構失敗自動分析、修復建議與重試

## 2. 架構設計

### 2.1 模組架構

```
antigravity/
├── Cargo.toml
├── SKILL.md
├── manifest.json
├── src/
│   ├── lib.rs              # 核心庫入口
│   ├── evolution_pipeline.rs  # 演化管線協調器
│   ├── discipline_guard.rs    # 開工紀律守門員
│   ├── consensus_ingest.rs    # 共識入庫引擎
│   ├── bottleneck_rca.rs      # 瓶頸根因分析
│   ├── code_health.rs         # 代碼健康檢查
│   ├── task_sync.rs           # 任務看板同步
│   ├── knowledge_sync.rs      # 知識庫同步
│   └── cicd_heal.rs           # CI/CD 自癒
```

### 2.2 核心資料流

```
Nostr Relay (Kind:30300) → Event Bridge → SQLite Store → Analysis Modules → Reports
                                    ↓
                              Buzz Channel (Thread)
                                    ↓
                              Obsidian (Second Brain)
```

### 2.3 關鍵類型定義

```rust
// 演化管線階段
enum PipelineStage {
    DisciplineGuard,    // 01
    ConsensusIngest,    // 02
    BottleneckRCA,      // 03
    CodeHealthCheck,    // 04
    TaskSync,           // 05
    KnowledgeSync,      // 06
    CicdSelfHeal,       // 07
}

// 演化報告
struct EvolutionReport {
    date: String,
    stages: Vec<StageResult>,
    summary: EvolutionSummary,
    metadata: ReportMetadata,
}
```

## 3. 實作計畫 (對應 DAILY_EVOLUTION_MASTER_IMPLEMENTATION_PLAN.md)

### 階段 1：核心基礎設施 (Week 1)
- [ ] `lib.rs` - 核心庫入口與公共類型
- [ ] `evolution_pipeline.rs` - 管線協調器，支援 `--skip`、`--only`、`--dry-run`
- [ ] `manifest.json` - 技能清單與 CLI 參數定義
- [ ] `Cargo.toml` - 依賴管理（nostr-sdk, rusqlite, serde, tokio, clap）

### 階段 2：分析模組實作 (Week 2)
- [ ] `discipline_guard.rs` - 移植並增強 `01_work_discipline_guard.py`
- [ ] `consensus_ingest.rs` - 移植並增強 `02_consensus_ingestion.py`
- [ ] `bottleneck_rca.rs` - 移植並增強 `03_bottleneck_rca.py`
- [ ] `code_health.rs` - 移植並增強 `04_code_health_check.py`

### 階段 3：同步與自癒模組 (Week 3)
- [ ] `task_sync.rs` - 移植並增強 `05_task_sync_board.py`
- [ ] `knowledge_sync.rs` - 移植並增強 `06_knowledge_sync.py`
- [ ] `cicd_heal.rs` - 移植並增強 `07_cicd_self_heal.py`

### 階段 4：整合測試與驗收 (Week 4)
- [ ] 單元測試覆蓋率 ≥ 80%
- [ ] 整合測試：`cargo test --lib`
- [ ] E2E 測試：`run_daily_evolution.py --dry-run` 全線走通
- [ ] Sentry 架構安全審核
- [ ] 主理人驗收簽核

## 4. 配置範例

```toml
# antigravity.toml
[nostr]
relay_url = "wss://presentyourlove.communities.buzz.xyz"
kind = 30300
private_key_env = "ANTIGRAVITY_NSEC"

[storage]
db_path = "./data/antigravity"
wal_mode = true

[pipeline]
default_stages = ["01", "02", "03", "04", "05", "06", "07"]
timeout_secs = 300

[monitor]
check_interval_secs = 3600  # 每小時檢查一次
alert_on_failure = true
alert_channel = "general"
```

## 5. CLI 介面

```bash
# 完整演化管線
antigravity --date 20261114

# 演練模式（不寫入資料庫、不發送事件）
antigravity --date 20261114 --dry-run

# 僅執行特定階段
antigravity --date 20261114 --only 01 04

# 跳過特定階段
antigravity --date 20261114 --skip 06 07

# 純 JSON 輸出（供自動化消費）
antigravity --date 20261114 --json-only
```

## 6. 輸出產物

| 檔案 | 說明 |
|------|------|
| `OUTBOX/DAILY_EVOLUTION_REPORT_YYYYMMDD.json` | 結構化完整報告 |
| `OUTBOX/DAILY_EVOLUTION_SUMMARY_YYYYMMDD.md` | 人類可讀摘要 |
| `PLANS/DAILY_EVOLUTION_PROPOSAL_YYYYMMDD.md` | 演化提案草稿 |
| `RESEARCH/RCA_YYYYMMDD.md` | 根因分析報告 |
| `GUIDES/CONSENSUS_YYYYMMDD.md` | 共識規範文件 |

## 7. 風險與對策

| 風險 | 可能性 | 影響 | 緩解措施 |
|------|--------|------|----------|
| Nostr Relay 斷線 | 中 | 高 | 重連邏輯、本機佇列緩衝、事件重發 |
| SQLite 鎖爭用 | 低 | 中 | WAL 模式、連線池、重試機制 |
| LLM 分析幻覺 | 中 | 高 | 結構化輸出驗證、Schema 限制、人工複核門禁 |
| 配置漂移 | 中 | 中 | 配置版本化、變更審計日誌、自動回滾 |

## 8. 監控指標

- `evolution_runs_total` - 累計執行次數
- `evolution_success_total` - 成功完成次數
- `evolution_failed_total` - 失敗次數
- `stage_duration_seconds` - 各階段耗時分布
- `nostr_events_published` - 發布事件數
- `consensus_items_ingested` - 入庫共識項目數
- `rca_findings_count` - RCA 發現數

## 9. 版本歷程

| 版本 | 日期 | 變更 |
|------|------|------|
| 1.0.0 | 2026-11-14 | 初始版本：架構設計與實作計畫 |

## 10. 簽署與審核

- **企劃**: Honey2
- **審核**: Sentry (架構安全官)
- **實作**: Pollen (核心技術主管)
- **歸檔**: Wax (第二大腦記憶官)

> **Sentry Audit**: PENDING — 待審核
> **主理人授權**: 2026-11-14 同意立項
