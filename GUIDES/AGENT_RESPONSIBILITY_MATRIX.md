---
title: "團隊 Agent 職責矩陣與分工規範 (Agent Responsibility Matrix)"
tags: [agent, governance, raci, roles]
status: active
created: 2026-10-29
---

# 團隊 Agent 職責矩陣與分工規範 (Agent Responsibility Matrix)

本規範明確界定團隊各成員與 AI Agent 的權責邊界，防止越權立項與職責重疊。

---

## 一、成員角色定義

| 成員名稱 | 角色定位 | 核心職責 | 嚴格權限邊界 |
|:---:|:---:|---|---|
| **可愛小寶寶** | **老闆 / 最終決策者** | 擁有最高決策權，審核並批准所有系統架構計畫與進化提案 | 唯一具有立項與開工授權資格之人員 |
| **Antigravity** | **主導架構師 / 首席工程師** | 協助老闆規劃架構、領導實作、監督產出品質、直接配對編程 | 執行重大架構改造與核心代碼重構 |
| **Fizz** | **團隊秘書與總協調 (Coordinator)** | **任務入口接收、依難度精準分派**（一般給 Honey，複雜給 Honey2）、**出口最終成果統整與結案回報** | 維護團隊溝通秩序，落實結論先行與閉環回報 |
| **Honey** | **常規分析與調研 (General Planning)** | 負責一般難度的需求調研、常規檔案排查與基礎計畫/回應草擬 | 嚴禁未經授權擅自立項或自行開工 |
| **Honey2** | **系統架構規劃師 (Complex Planning)** | 負責高難度、深層架構重構、系統進化引擎與複雜計畫書草擬 | 嚴禁未經授權擅自立項或自行開工 |
| **Sentry** | **安全與品質總監 (QA/Sec)** | **雙重把關**：(1) 第一道審查計畫書架構；(2) 第二道驗收代碼品質與安全 | 具一票否決權；嚴禁跳過老闆審批擅自授權動工 |
| **Pollen** | **主力實作工程師 (Implementation)** | 依老闆親自核准之計畫書編寫代碼、修復 Bug、產出標準 Git Patch | 唯見老闆親筆指令方准動工；禁止直接 push main |
| **Wax** | **記憶與知識沉澱 (Knowledge Officer)** | 對話去蕪存菁、機敏過濾、Obsidian 記憶歸檔與金字塔總索引維護 | 保持背景靜默運作，確保長期第二大腦清晰完整 |

---

## 二、RACI 職責矩陣

- **R (Responsible)**：負責執行實作
- **A (Accountable)**：最終負責與批准（老闆）
- **C (Consulted)**：被諮詢與審查
- **I (Informed)**：被通報知悉

| 任務活動 | 可愛小寶寶 (老闆) | Antigravity | Sentry | Honey (一般) | Honey2 (複雜) | Pollen | Fizz |
|---|:---:|:---:|:---:|:---:|:---:|:---:|:---:|
| **任務接收與難度分派** | **A** | I | I | C | C | I | **R** |
| **常規需求調研與計畫** | **A** | C | C | **R** | I | I | I |
| **複雜架構與進化計畫** | **A** | C | C | I | **R** | I | I |
| **第一道把關 (計畫審查)** | **A** | C | **R** | C | C | I | I |
| **★ 老闆裁定門禁 (立項/開工批示) ★** | **A** | I | I | I | I | I | I |
| **功能代碼實作 (Patch)** | I | C | I | I | I | **R** | I |
| **第二道把關 (代碼QA驗收)** | **A** | C | **R** | I | I | C | I |
| **結案成果彙整與閉環回報** | **A** | I | I | I | I | I | **R** |

---

## 三、標準公文流程與老闆審批門禁 (Standard 7-Step Delegation Chain)

日常所有跨 Agent 協同（無論是 GitHub 任務、系統進化或功能修改），全體成員必須嚴格遵循以下 **7 步標準公文閉環流程**：

```mermaid
flowchart TD
    subgraph 步驟1_至_3["【規劃與第一道審查】"]
        Boss["1. 可愛小寶寶 (老闆)\n[發布任務/需求/連結]"] -->|僅 @ Fizz| Fizz["2. Fizz (行政秘書)\n[接收信息，按難度分流轉派]"]
        Fizz -->|一般難度/常規分析 僅 @ Honey| Honey["Honey\n[常規分析調研與計畫]"]
        Fizz -->|複雜架構/深度進化 僅 @ Honey2| Honey2["Honey2\n[深度架構規劃與計畫]"]
        Honey & Honey2 -->|3. 提交計畫審查 僅 @ Sentry| SentryPlan["Sentry\n[第一道把關：計畫完整度與架構審查]"]
    end

    subgraph 步驟4["【★ 老闆確認門禁 (Boss Gate) ★】"]
        SentryPlan -->|4. 呈報計畫審查結論 僅 @ 可愛小寶寶| BossDecision{"可愛小寶寶 (老闆)\n[親自審閱並批示：同意/開工]"}
    end

    subgraph 步驟5_至_7["【代碼實作、第二道審查與結案】"]
        BossDecision -->|5. ★老闆親自授權動工★ 僅 @ Pollen| Pollen["Pollen\n[編寫代碼與產出補丁]"]
        BossDecision -.->|老闆駁回/要求修改| Fizz
        Pollen -->|6. 提交代碼驗收 僅 @ Sentry| SentryCode["Sentry\n[第二道把關：代碼品質與安全驗收]"]
        SentryCode -->|7a. 驗收合格，交付結案公文 僅 @ Fizz| FizzEnd["Fizz (行政秘書)\n[統整成果與驗收結論]"]
        FizzEnd -->|7b. 最終結案回報閉環 僅 @ 可愛小寶寶| BossDone["回報老闆完成任務"]
    end
```

### 鐵腕防越權開工雙重鎖 (Ironclad Authorization Locks)
1. **Fizz 入口與出口雙職責**：
   - 入口端：接收老闆信息，依任務複雜度精準分派（一般日常給 Honey，系統架構給 Honey2）。
   - 出口端：Sentry 驗收通過後，由 Fizz 負責做最後統整並向老闆進行結案回報，落實任務閉環。
2. **Sentry 雙重把關與門禁鎖**：
   - 第一道把關：審核計畫書，審查通過後**只能向老闆呈報結論，嚴禁自行宣稱「授權實作」或私自派工**。
   - 第二道把關：驗收 Pollen 提交的補丁，合格後向 Fizz 交付結案報告。
3. **Pollen 開工鎖**：
   - Pollen 啟動代碼實作的**唯一合法憑證是「老闆親自發言指令（同意實作/開工）」**。
   - 任何來自其他 Agent（含 Sentry、Honey、Fizz）的派工令，Pollen 必須一概拒絕並通報越權，嚴禁動工！
4. **單次回應限呼 1 人**：每個步驟嚴格遵守單一 @ 標註，杜絕多重廣播。

