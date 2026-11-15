/**
 * 對話脈絡解析模組 (Conversation Parser)
 * 負責從頻道與歷史紀錄中識別並聚類 Thread 討論串
 */

export class ConversationParser {
  constructor(config = {}) {
    this.channelId = config.buzz?.channel_id || '5ca019f6-1a27-592a-b015-4baf0eb33572';
    this.channelName = config.buzz?.channel_name || 'general';
  }

  /**
   * 取得已識別之所有對話討論串資料
   * @returns {Array<object>} Thread 清單
   */
  getHistoricalThreads() {
    return [
      {
        id: '2026-10-24_ui-localization',
        threadRoot: 'th_loc_20261024_01',
        title: 'Buzz 介面繁體中文在地化開工討論',
        dateRange: '2026-10-24 16:48 ~ 17:30',
        messageCount: 3,
        participants: ['可愛小寶寶', 'Pollen', 'Honey'],
        tags: ['localization', 'ui', 'traditional-chinese'],
        background: '老闆下達將 Buzz Desktop 與前端介面從英文全面翻譯為繁體中文之任務，要求依據 Honey 撰寫的實施計畫書執行。',
        ownerDecisions: [
          '所有介面文字採用繁體中文（台灣慣用語標準）。',
          '必須在代碼庫直接落檔，修改後產出 Git Patch 補丁。',
        ],
        architectureConsensus: [
          '由 Honey 產出 `UI_TRADITIONAL_CHINESE_LOCALIZATION_PLAN.md`。',
          '修改側邊欄元件 `AppSidebar.tsx` 與頂部列 `AppSidebarPinnedHeader.tsx`。',
        ],
        actionItems: [
          '完成側邊欄繁體中文語系切換。',
          '產出 `UI_TRADITIONAL_CHINESE_LOCALIZATION.patch`。',
        ],
      },
      {
        id: '2026-10-24_context-digestion-gateway',
        threadRoot: 'th_cdg_20261024_02',
        title: '上下文消化閘道 (Context Digestion Gateway) 與 Phase 2/3 架構研討',
        dateRange: '2026-10-24 18:51 ~ 2026-10-25 03:00',
        messageCount: 41,
        participants: ['可愛小寶寶', 'Pollen', 'Honey', 'Fizz'],
        tags: ['context-digestion', 'rag', 'architecture', 'action-sink'],
        background: '針對頻道對話過長導致 Agent 頻繁遭遇 Token 爆炸與 429 限流問題，進行底層消化閘道與 RAG 增強機制之深入調研與架構規劃。',
        ownerDecisions: [
          '【代碼實作鐵律】：嚴禁只寫 Markdown 偽裝完工，所有開發必須修改專案原始碼並產出可用 Patch。',
          '【本機路徑唯一標準】：Buzz 主專案路徑永遠為 `c:\\私人\\ai agent\\buzz`。',
          '【防吞字鐵律】：嚴禁使用角括號 `< >` 包裹任何檔名或路徑，一律使用 Markdown 反引號或代碼區塊。',
        ],
        architectureConsensus: [
          '實作 Phase 2 原生 ActionSink `DigestContext` 於 Rust 工作流引擎 (`buzz-workflow`)。',
          '實作 Phase 3 上下文 RAG 增強 Hook `useAgentContextEnrichment.ts`。',
          '明確 Agent 開工門禁：Pollen 必須在老闆明確下達開工指令後方可動工，嚴禁未讀檔盲目表態。',
        ],
        actionItems: [
          '產出 `PHASE2_NATIVE_DIGEST.patch`。',
          '產出 `PHASE3_RAG_ENRICHMENT.patch`。',
        ],
      },
      {
        id: '2026-10-25_mobile-control-feasibility',
        threadRoot: 'th_mob_20261025_01',
        title: 'Buzz 行動端遠端控制可行性探討',
        dateRange: '2026-10-25 10:07 ~ 10:30',
        messageCount: 4,
        participants: ['可愛小寶寶', 'Honey'],
        tags: ['mobile', 'remote', 'whatsapp', 'relay'],
        background: '老闆詢問是否能透過手機 App 或通訊軟體（如 WhatsApp）遠端向 Buzz 代理人發送指令並接收進度回報。',
        ownerDecisions: [
          '列入中遠期規劃，現階段優先穩固桌面端與 Obsidian 記憶系統。',
        ],
        architectureConsensus: [
          '可透過 Nostr Relay 行動端客戶端或 Webhook Gateway 轉接。',
          '外網通訊需嚴格遵循安全端到端加密防線。',
        ],
        actionItems: [
          '將行動端同步方案納入長期架構願景。',
        ],
      },
      {
        id: '2026-10-25_obsidian-second-brain-integration',
        threadRoot: 'th_obs_20261025_02',
        title: 'Obsidian 第二大腦記憶整合與 Wax 角色誕生',
        dateRange: '2026-10-25 11:24 ~ 15:30',
        messageCount: 22,
        participants: ['可愛小寶寶', 'Fizz', 'Honey', 'Pollen', 'Wax'],
        tags: ['obsidian', 'second-brain', 'wax', 'memory-rules'],
        background: '老闆提出將 Buzz 對話與長期記憶自動沉澱為 Obsidian Vault Markdown 知識庫的重大構想，並親自完成五大核心規格確認。',
        ownerDecisions: [
          '【Vault 本機路徑】：固定為 `c:\\私人\\ai agent\\buzz-obsidian`。',
          '【Git 版本控制】：純本機 Git 進行版本追蹤即可，暫不上傳雲端 Remote。',
          '【同步頻率】：Core Memory 5 分鐘、Cold Memory 10 分鐘、對話摘要 30 分鐘。',
          '【敏感標籤過濾】：凡帶有 `#private`、`#secret`、`#tmp`、`#nosync`、`#個人` 標籤完全不予寫入。',
          '【新 Agent 設立】：正式任命第四位 Agent「Wax（記憶清潔工）」，專職對話去蕪存菁與雙向連結提煉。',
        ],
        architectureConsensus: [
          '建立標準檔案架構（`00-Core-Memory`、`01-Cold-Memory`、`02-Conversation-Digest`、`03-Entities`）。',
          '制定 `c:\\私人\\ai agent\\buzz-obsidian\\歡迎.md` 作為最高保管庫守則。',
          'Wax 模型採用 OpenRouter 免費之 `nvidia/nemotron-3-ultra-550b-a55b:free`。',
        ],
        actionItems: [
          '產出 `OBSIDIAN_MEMORY_SYNC.md` 實施計畫書。',
          '建立 `c:\\私人\\ai agent\\buzz-obsidian` Vault 基礎結構。',
          '在 Buzz Desktop 註冊 Wax 代理人並完成提示詞綁定。',
        ],
      },
      {
        id: '2026-10-25_m1-m2-m3-execution-and-verification',
        threadRoot: 'th_m123_20261025_03',
        title: 'Pollen 429 限流調查、M1 骨架交付與 M2/M3 落地推進',
        dateRange: '2026-10-25 15:51 ~ 16:50',
        messageCount: 15,
        participants: ['可愛小寶寶', 'Pollen', 'Antigravity'],
        tags: ['pollen-429', 'm1-sync', 'm2-cold', 'm3-digest', 'delivery'],
        background: 'Pollen 因舊對話長度觸發 Google 250k TPM 限流錯誤，隨後在重試時陷入「文字假完工」盲點；經詳細調查後，由 Antigravity 正式接手執行端到端實作。',
        ownerDecisions: [
          '確認 Pollen 未落檔之事實，全權授權 Antigravity 實作 M1、M2 與 M3 模組。',
          '要求確保所有 Agent 能夠完整讀取整理過的核心記憶、冷記憶與歷史對話摘要。',
        ],
        architectureConsensus: [
          '由於本機未安裝 Python，統一採用 Node.js v24 原生 ES Modules 實作，免外部依賴。',
          'M1 成功落地 Core Memory 覆寫同步與 Obsidian 本地 Git 自動化追蹤。',
          'M2 實作 Cold Memory 增量提取與自動生成 `01-Cold-Memory/INDEX.md`。',
          'M3 實作對話脈絡語意提煉與自動生成 `02-Conversation-Digest/INDEX.md`。',
        ],
        actionItems: [
          '產出 `M1_CORE_MEMORY_SYNC.patch` 並存至 OUTBOX。',
          '完成 M2 + M3 完整整合測試與交付。',
        ],
      },
    ];
  }

  /**
   * 將動態抓取到的即時訊息依據「日期」與「討論串 (Thread)」進行分群與語意抽取
   * @param {Array<object>} messages - Nostr 訊息物件陣列
   * @returns {Map<string, Array<object>>} DateString (YYYY-MM-DD) -> Array of clustered Threads
   */
  clusterLiveMessages(messages) {
    if (!messages || messages.length === 0) return new Map();

    const dateMap = new Map();

    for (const msg of messages) {
      const d = new Date(msg.created_at * 1000);
      const dateStr = d.toLocaleDateString('en-CA', { timeZone: 'Asia/Taipei' });
      if (!dateMap.has(dateStr)) {
        dateMap.set(dateStr, []);
      }
      dateMap.get(dateStr).push(msg);
    }

    const resultMap = new Map();

    for (const [dateStr, dayMsgs] of dateMap.entries()) {
      dayMsgs.sort((a, b) => a.created_at - b.created_at);
      const threadGroups = new Map();

      for (const m of dayMsgs) {
        let threadKey = null;
        for (const t of (m.tags || [])) {
          if (t[0] === 'e' && (t[3] === 'root' || t[3] === 'reply')) {
            threadKey = t[1];
            break;
          }
        }
        if (!threadKey) {
          threadKey = m.id;
        }

        if (!threadGroups.has(threadKey)) {
          threadGroups.set(threadKey, []);
        }
        threadGroups.get(threadKey).push(m);
      }

      const threads = [];
      for (const [threadId, tMsgs] of threadGroups.entries()) {
        const firstMsg = tMsgs[0];
        const lastMsg = tMsgs[tMsgs.length - 1];
        const participants = Array.from(new Set(tMsgs.map(m => m.author_name || m.pubkey.slice(0, 8))));

        let title = firstMsg.content.trim().split('\n')[0].replace(/^@\S+\s*/, '').slice(0, 40);
        if (!title) title = `討論串 ${threadId.slice(0, 8)}`;

        const startTime = new Date(firstMsg.created_at * 1000).toLocaleTimeString('zh-TW', { timeZone: 'Asia/Taipei', hour: '2-digit', minute: '2-digit' });
        const endTime = new Date(lastMsg.created_at * 1000).toLocaleTimeString('zh-TW', { timeZone: 'Asia/Taipei', hour: '2-digit', minute: '2-digit' });

        const ownerDecisions = [];
        for (const m of tMsgs) {
          if (m.is_owner) {
            const lines = m.content.split('\n').map(l => l.trim()).filter(Boolean);
            for (const line of lines) {
              if (line.includes('記低') || line.includes('以後') || line.includes('嚴禁') || line.includes('必須') || line.includes('同意') || line.includes('開工') || line.includes('規則')) {
                ownerDecisions.push(line.replace(/^@\S+\s*/, ''));
              }
            }
          }
        }

        const tags = ['live-digest', 'general'];
        if (tMsgs.some(m => m.content.includes('workflow') || m.content.includes('工作流'))) tags.push('workflows');
        if (tMsgs.some(m => m.content.includes('角括號') || m.content.includes('吞字'))) tags.push('anti-swallow-rule');
        if (tMsgs.some(m => m.content.includes('Apify') || m.content.includes('爬蟲'))) tags.push('apify');

        threads.push({
          id: `${dateStr}_th_${threadId.slice(0, 8)}`,
          threadRoot: threadId,
          title,
          dateRange: `${dateStr} ${startTime} ~ ${endTime}`,
          messageCount: tMsgs.length,
          participants,
          tags,
          background: firstMsg.content.slice(0, 150),
          ownerDecisions: ownerDecisions.length > 0 ? ownerDecisions : ['頻道交流與需求探討。'],
          messages: tMsgs,
        });
      }

      resultMap.set(dateStr, threads);
    }

    return resultMap;
  }
}
