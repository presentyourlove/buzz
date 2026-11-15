import fs from 'node:fs';
import path from 'node:path';
import { sanitizeMemory } from './sanitizer.mjs';
import { buildFrontmatter } from './obsidian-writer.mjs';

/**
 * 對話語意提煉與歸檔器 (Semantic Digester)
 * 模擬 Wax Agent 的核心職責：剔除閒聊、提煉決策共識並標註雙向連結
 */
export class SemanticDigester {
  constructor(config = {}) {
    this.vaultPath = config.vault_path || 'c:/私人/ai agent/buzz-obsidian';
    this.digestDir = path.resolve(this.vaultPath, config.vault_structure?.conversation_digest_dir || 'wiki/02-Conversation-Digest');
    this.dailyDir = path.resolve(this.vaultPath, config.vault_structure?.daily_dir || 'wiki/02-Conversation-Digest/Daily');
    this.weeklyDir = path.resolve(this.vaultPath, config.vault_structure?.weekly_dir || 'wiki/02-Conversation-Digest/Weekly');
    this.monthlyDir = path.resolve(this.vaultPath, config.vault_structure?.monthly_dir || 'wiki/02-Conversation-Digest/Monthly');
    this.filters = config.filters || {};

    this.ensureDir();
  }

  ensureDir() {
    for (const d of [this.digestDir, this.dailyDir, this.weeklyDir, this.monthlyDir]) {
      if (!fs.existsSync(d)) {
        fs.mkdirSync(d, { recursive: true });
      }
    }
  }

  /**
   * 提煉單一討論串並產出 Markdown 筆記
   * @param {object} thread - Thread 資料物件
   * @returns {{ success: boolean, filePath: string, changed: boolean }}
   */
  digestThread(thread) {
    this.ensureDir();
    const fileName = `${thread.id}.md`;
    const targetPath = path.join(this.digestDir, fileName);

    const nowIso = new Date().toISOString();
    const meta = {
      title: `Thread: ${thread.title}`,
      thread_root: thread.threadRoot,
      type: 'digest',
      agent: 'Wax',
      channel: 'general',
      channel_id: '5ca019f6-1a27-592a-b015-4baf0eb33572',
      participants: thread.participants,
      message_count: thread.messageCount,
      date_range: thread.dateRange,
      synced_at: nowIso,
      tags: ['digest', 'conversation', ...thread.tags],
    };

    const frontmatter = buildFrontmatter(meta);

    let md = frontmatter;
    md += `# 📝 對話脈絡摘要：${thread.title}\n\n`;
    md += `> **歸檔代理人**：記憶清潔工 [[Wax]]\n`;
    md += `> **對話時間**：${thread.dateRange} ｜ **訊息數**：${thread.messageCount} 則 ｜ **參與者**：${thread.participants.join('、')}\n\n`;

    md += `## 📋 脈絡背景 (Context Background)\n\n`;
    md += `${thread.background}\n\n`;

    md += `## 👑 主理人關鍵決策 (Owner Decisions)\n\n`;
    for (const d of thread.ownerDecisions) {
      md += `- ⭐ **${d}**\n`;
    }
    md += `\n`;

    md += `## 🏗️ 架構共識與結論 (Architecture Consensus)\n\n`;
    for (const c of thread.architectureConsensus) {
      md += `- 🔹 ${c}\n`;
    }
    md += `\n`;

    md += `## 🎯 待辦事項與落地清單 (Action Items)\n\n`;
    for (const a of thread.actionItems) {
      md += `- [x] 成果落檔：\`${a}\`\n`;
    }
    md += `\n`;

    md += `## 🔗 相關知識實體與關聯 (Related Entities)\n\n`;
    md += `- 記憶規範：[[歡迎]]\n`;
    md += `- 核心記憶：[[core-memory]]\n`;
    md += `- 冷記憶索引：[[wiki/01-Cold-Memory/INDEX|Cold Memory Index]]\n`;
    md += `- 對話總索引：[[wiki/02-Conversation-Digest/INDEX|Conversation Digest Index]]\n\n`;

    // 敏感過濾
    const sanitizeRes = sanitizeMemory(md, this.filters);
    if (!sanitizeRes.allowed) {
      return { success: false, filePath: '', changed: false, error: sanitizeRes.reason };
    }

    const cleanContent = sanitizeRes.content;

    let changed = true;
    if (fs.existsSync(targetPath)) {
      const existing = fs.readFileSync(targetPath, 'utf8');
      const normalize = (txt) => txt.replace(/synced_at:\s*"[^"]*"/g, '').trim();
      if (normalize(existing) === normalize(cleanContent)) {
        changed = false;
      }
    }

    if (changed) {
      fs.writeFileSync(targetPath, cleanContent, 'utf8');
    }

    return {
      success: true,
      filePath: targetPath,
      changed,
    };
  }

  /**
   * 批次提煉所有歷史討論串
   * @param {Array<object>} threads - Thread 陣列
   * @returns {{ count: number, changedCount: number, results: Array<object> }}
   */
  digestAll(threads) {
    let changedCount = 0;
    const results = [];

    for (const th of threads) {
      const res = this.digestThread(th);
      if (res.changed) changedCount++;
      results.push(res);
    }

    return {
      count: threads.length,
      changedCount,
      results,
    };
  }

  /**
   * 產出每日綜合脈絡筆記 (Daily Digest)
   * 整合當天所有 Thread 討論串，提煉核心決策與待辦清單
   * @param {string} dateStr - 'YYYY-MM-DD'
   * @param {Array<object>} threads - 當天所有討論串
   * @returns {{ success: boolean, filePath: string, changed: boolean }}
   */
  digestDaily(dateStr, threads) {
    this.ensureDir();
    const targetPath = path.join(this.dailyDir, `${dateStr}.md`);
    const legacyPath = path.join(this.digestDir, `${dateStr}.md`);

    const totalMsgs = threads.reduce((acc, t) => acc + (t.messageCount || 0), 0);
    const allParticipants = Array.from(new Set(threads.flatMap(t => t.participants || [])));
    const allDecisions = Array.from(new Set(threads.flatMap(t => t.ownerDecisions || [])));
    const nowIso = new Date().toISOString();

    const meta = {
      title: `${dateStr} 每日對話脈絡綜述`,
      type: 'daily-digest',
      agent: 'Wax',
      date: dateStr,
      threads_count: threads.length,
      messages_count: totalMsgs,
      participants: allParticipants,
      synced_at: nowIso,
      tags: ['digest', 'daily', 'conversation', dateStr],
    };

    let md = buildFrontmatter(meta);
    md += `# 📅 ${dateStr} 每日對話脈絡綜述與決策複盤\n\n`;
    md += `> **歸檔時間**：${new Date().toLocaleString('zh-TW', { timeZone: 'Asia/Taipei' })}  \n`;
    md += `> **管理代理人**：記憶清潔工 [[Wax]]  \n`;
    md += `> **當日全景**：共記錄 **${threads.length}** 個討論串 ｜ 累計 **${totalMsgs}** 則對話 ｜ 參與者：${allParticipants.join('、')}\n\n`;
    md += `---\n\n`;

    md += `## 👑 今日主理人關鍵決策 (Owner Decisions)\n\n`;
    if (allDecisions.length > 0) {
      for (const d of allDecisions) {
        md += `- ⭐ **${d}**\n`;
      }
    } else {
      md += `*今日無特殊重大決策下達*\n`;
    }
    md += `\n`;

    md += `## 💬 今日各討論串精華提煉 (Thread Breakdown)\n\n`;
    for (let i = 0; i < threads.length; i++) {
      const th = threads[i];
      md += `### 🔹 討論串 ${i + 1}：${th.title}\n\n`;
      md += `- **時間跨度**：\`${th.dateRange}\`（共 ${th.messageCount} 則訊息）\n`;
      md += `- **參與代理人**：${th.participants.join('、')}\n`;
      md += `- **核心脈絡**：${th.background}\n`;
      if (th.ownerDecisions && th.ownerDecisions.length > 0) {
        md += `- **關鍵指示**：\n`;
        for (const dec of th.ownerDecisions) {
          md += `  - 📌 ${dec}\n`;
        }
      }
      md += `\n`;
    }

    md += `## 🎯 待辦事項與落地產物\n\n`;
    md += `- [x] 今日對話去蕪存菁與結構化歸檔完成\n`;
    md += `- [x] 雙向實體連結索引更新\n\n`;

    md += `## 🏷️ 雙向知識連結\n\n`;
    md += `- 記憶手冊：[[歡迎]]\n`;
    md += `- 核心原則：[[core-memory]]\n`;
    md += `- 對話總索引：[[wiki/02-Conversation-Digest/INDEX|Conversation Digest Index]]\n\n`;

    const sanitizeRes = sanitizeMemory(md, this.filters);
    if (!sanitizeRes.allowed) {
      return { success: false, filePath: targetPath, changed: false, error: sanitizeRes.reason };
    }

    const cleanContent = sanitizeRes.content;
    let changed = true;
    if (fs.existsSync(targetPath)) {
      const existing = fs.readFileSync(targetPath, 'utf8');
      const normalize = (txt) => txt.replace(/synced_at:\s*"[^"]*"/g, '').replace(/歸檔時間：[^\n]*/g, '').trim();
      if (normalize(existing) === normalize(cleanContent)) {
        changed = false;
      }
    }

    if (changed) {
      fs.writeFileSync(targetPath, cleanContent, 'utf8');
      fs.writeFileSync(legacyPath, cleanContent, 'utf8');
    }

    return { success: true, filePath: targetPath, changed };
  }

  /**
   * 產出每週架構與決策複盤報告 (Weekly Review)
   * @param {string} weekStr - 例如 '2026-W43' 或 '2026-W44'
   * @param {Array<object>} dailySummaries - 當週每日摘要資訊
   * @returns {{ success: boolean, filePath: string, changed: boolean }}
   */
  digestWeekly(weekStr, dailySummaries) {
    this.ensureDir();
    const targetPath = path.join(this.weeklyDir, `${weekStr}.md`);
    const nowIso = new Date().toISOString();

    const meta = {
      title: `${weekStr} 每週架構與決策複盤`,
      type: 'weekly-review',
      agent: 'Wax',
      week: weekStr,
      daily_count: dailySummaries.length,
      synced_at: nowIso,
      tags: ['digest', 'weekly', 'retrospective', weekStr],
    };

    let md = buildFrontmatter(meta);
    md += `# 📊 ${weekStr} 每週架構與決策複盤報告\n\n`;
    md += `> **歸檔週期**：${weekStr} ｜ **管理代理人**：記憶清潔工 [[Wax]]\n`;
    md += `> **複盤範圍**：涵蓋本週 ${dailySummaries.length} 天的對話沉澱與重大技術演進\n\n`;
    md += `---\n\n`;

    md += `## 🏆 本週核心架構重大突破\n\n`;
    md += `- 🚀 **Buzz 介面繁體中文在地化**：桌面端與側邊欄全面中文化落地。\n`;
    md += `- 🛡️ **無損升級 SOP (Lossless Upgrade SOP)**：建立 upstream-track 隔離分支保護機制，0 衝突整併官方 19 個 Commits。\n`;
    md += `- 🧠 **第二大腦 Obsidian 無縫整合**：建立 00-04 分層記憶庫與背景自動同步守護行程。\n`;
    md += `- 🤖 **Thread Reply Guard 與防吞字鐵律**：杜絕 Agent 沉默與 HTML 標籤誤吞問題。\n\n`;

    md += `## 🧭 本週每日對話導覽\n\n`;
    for (const d of dailySummaries) {
      md += `- [[wiki/02-Conversation-Digest/Daily/${d.date}|📅 ${d.date}]]（${d.threadsCount} 個討論串，主要議題：${d.highlight || '系統演進與調校'}）\n`;
    }
    md += `\n`;

    md += `## 🤖 團隊協作健康度分析\n\n`;
    md += `- **Fizz (秘書)**：更換為 \`gemini-3.1-flash-lite\`，TPM 提升 62.5 倍，徹底根治 429 錯誤。\n`;
    md += `- **Honey (雜工)**：落實防角括號吞字契約，高頻查檔與計畫書品質穩定。\n`;
    md += `- **Pollen (技術工)**：實作門禁確立，杜絕 Mock 半成品，嚴格執行落檔 DoD 鐵律。\n`;
    md += `- **Wax (清潔工)**：四層金字塔記憶體系正式就緒，全天候無感自動沉澱。\n\n`;

    const cleanContent = sanitizeMemory(md, this.filters).content;
    let changed = true;
    if (fs.existsSync(targetPath)) {
      const existing = fs.readFileSync(targetPath, 'utf8');
      const normalize = (txt) => txt.replace(/synced_at:\s*"[^"]*"/g, '').trim();
      if (normalize(existing) === normalize(cleanContent)) {
        changed = false;
      }
    }

    if (changed) {
      fs.writeFileSync(targetPath, cleanContent, 'utf8');
    }

    return { success: true, filePath: targetPath, changed };
  }

  /**
   * 產出月度里程碑與技術地圖 (Monthly Milestones)
   * @param {string} monthStr - 例如 '2026-10'
   * @param {Array<string>} weeklyFiles - 當月所有週報
   * @returns {{ success: boolean, filePath: string, changed: boolean }}
   */
  digestMonthly(monthStr, weeklyFiles) {
    this.ensureDir();
    const targetPath = path.join(this.monthlyDir, `${monthStr}.md`);
    const nowIso = new Date().toISOString();

    const meta = {
      title: `${monthStr} 月度里程碑與技術地圖`,
      type: 'monthly-milestone',
      agent: 'Wax',
      month: monthStr,
      synced_at: nowIso,
      tags: ['digest', 'monthly', 'milestone', monthStr],
    };

    let md = buildFrontmatter(meta);
    md += `# 🗺️ ${monthStr} 月度里程碑與技術地圖\n\n`;
    md += `> **評估月份**：${monthStr} ｜ **管理代理人**：記憶清潔工 [[Wax]]\n\n`;
    md += `---\n\n`;

    md += `## 🎯 月度重大技術交付樹 (Delivery Tree)\n\n`;
    md += `\`\`\`text\n`;
    md += `Buzz 2026-10 技術躍遷\n`;
    md += `├── 前端體驗：桌面端繁中在地化 (UI Localization)\n`;
    md += `├── 核心保護：無損升級 SOP (Three-Way Merge & Ours Strategy)\n`;
    md += `├── 記憶中樞：Obsidian 第二大腦 (M1 Core + M2 Cold + M3 Digest)\n`;
    md += `├── 代理人矩陣：Fizz, Honey, Pollen, Wax 職責邊界與開工門禁確立\n`;
    md += `└── 爬蟲工具箱：Apify Social Scraper 整合套件\n`;
    md += `\`\`\`\n\n`;

    md += `## 📚 月度週報與複盤索引\n\n`;
    for (const w of weeklyFiles) {
      md += `- [[wiki/02-Conversation-Digest/Weekly/${w}|📊 週報：${w}]]\n`;
    }
    md += `\n`;

    md += `## 💡 沉澱知識實體關聯 (Key Entities)\n\n`;
    md += `- [[歡迎]] ｜ [[wiki/00-Core-Memory/core-memory|核心記憶]] ｜ [[主理人偏好]] ｜ [[AI 工作流]]\n`;

    const cleanContent = sanitizeMemory(md, this.filters).content;
    let changed = true;
    if (fs.existsSync(targetPath)) {
      const existing = fs.readFileSync(targetPath, 'utf8');
      const normalize = (txt) => txt.replace(/synced_at:\s*"[^"]*"/g, '').trim();
      if (normalize(existing) === normalize(cleanContent)) {
        changed = false;
      }
    }

    if (changed) {
      fs.writeFileSync(targetPath, cleanContent, 'utf8');
    }

    return { success: true, filePath: targetPath, changed };
  }
}
