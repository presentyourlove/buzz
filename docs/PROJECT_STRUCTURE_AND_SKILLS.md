# Buzz 專案結構樹與可用技能導覽 (Project Structure & Skills Catalog)

> **版本**：1.0.0 (Phase 59)  
> **適用範圍**：特工工作目錄導航、代碼定位與技能調用。

---

## 1. 專案核心目錄結構樹 (Project Structure Tree)

特工執行時工作目錄為 `c:\私人\ai agent\buzz\`：

```text
c:\私人\ai agent\buzz\
├── .agents/skills/              # 【技能目錄】特工運行時可動態載入之 Skill（優先掃描）
│   ├── collab-protocol/         # 多特工協同工作流協議
│   ├── comm-quality-guard/      # 通訊格式清理與防食字衛士 (Python)
│   ├── daily-digest/            # 每日摘要日誌生成技能 (Rust)
│   ├── desktop-screenshot/      # 桌面截圖輔助技能
│   ├── env-guardian/            # 開發環境與依賴守護 (Rust)
│   ├── qa-guard/                # 代碼品質與語法門禁檢查 (Rust)
│   └── sprout-cli/              # CLI 擴充技能
├── crates/                      # 【核心生產代碼】Rust 工作區 crates（嚴禁企劃/秘書直接修改）
│   ├── buzz-acp/                # Agent 控制面板 (RBAC 沙箱、進程池)
│   ├── buzz-agent/              # 特工運行底層 (Triggers 8080 排程器、Triple-Gate 門禁)
│   ├── buzz-cli/                # buzz 命令列主程式
│   ├── buzz-core/               # 核心資料型別與事件 kind 定義
│   └── buzz-persona/            # 特工 Persona Pack 規範與解析器
├── dashboard/                   # 【監控儀表板】Vue 3 + Vite 4173 即時動態遙測 Web UI
├── docs/                        # 【權威架構文檔】
│   ├── GLOBAL_AGENT_PROMPT.md   # 全域特工核心規範與底層清冊
│   ├── PROJECT_STRUCTURE_AND_SKILLS.md # 本導航手冊
│   ├── SYSTEM_CAPABILITIES_REALITY.md  # 系統真實家底清冊
│   └── AGENT_ANTIGRAVITY_COLLABORATION_SOP.md # 協同 SOP
├── PLANS/                       # 【企劃產出目錄】Honey2 / Honey 撰寫之 Markdown 計畫書
├── RESEARCH/                    # 【調研存檔目錄】技術情報、架構評估報告
└── skills/                      # 【技能源碼庫】技能模組實體開發目錄
```

---

## 2. 可用技能清單與調用方式 (Skills Catalog)

特工可透過 `load_skill(name)` 動態載入以下技能指令：

| 技能名稱 (`load_skill`) | 實裝語言 | 核心用途與觸發時機 |
| :--- | :---: | :--- |
| **`comm-quality-guard`** | Python | 訊息發送前清理角括號與行首標點，根治 Fizz 食字問題。 |
| **`qa-guard`** | Rust | 代碼交付前檢查 `cargo check`、編碼格式與修復建議。 |
| **`env-guardian`** | Rust | 檢查本機編譯工具鏈、環境變數與依賴健康狀況。 |
| **`collab-protocol`** | YAML/Python | 規範特工多工排程與跨階段狀態流轉。 |
| **`daily-digest`** | Rust | 聚合每日頻道日誌與產出 Markdown 進化報告。 |
| **`desktop-screenshot`** | Shell/Tool | 在需要視覺除錯時截取桌面狀態。 |

---

## 3. 工作前路徑安全約制
1. 嚴禁在未確認路徑前嘗試寫入不存在的子目錄。
2. 企劃類產出僅得寫入 `PLANS/` 或 `RESEARCH/`。
3. 程式碼修改僅限授權工程師特工在指定 `skills/` 或經主理人核准之模組內實施。
