import fs from 'node:fs';
import path from 'node:path';
import { buildFrontmatter } from './obsidian-writer.mjs';

/**
 * 對話脈絡摘要總索引建置器（支援四層金字塔架構）
 */
export class DigestIndexBuilder {
  constructor(config = {}) {
    this.vaultPath = config.vault_path || 'c:/私人/ai agent/buzz-obsidian';
    this.digestDir = path.resolve(this.vaultPath, config.vault_structure?.conversation_digest_dir || 'wiki/02-Conversation-Digest');
    this.dailyDir = path.resolve(this.vaultPath, config.vault_structure?.daily_dir || 'wiki/02-Conversation-Digest/Daily');
    this.weeklyDir = path.resolve(this.vaultPath, config.vault_structure?.weekly_dir || 'wiki/02-Conversation-Digest/Weekly');
    this.monthlyDir = path.resolve(this.vaultPath, config.vault_structure?.monthly_dir || 'wiki/02-Conversation-Digest/Monthly');
    this.indexPath = path.resolve(this.digestDir, 'INDEX.md');
  }

  scanDir(dirPath) {
    if (!fs.existsSync(dirPath)) return [];
    return fs.readdirSync(dirPath)
      .filter(f => f.endsWith('.md') && f !== 'INDEX.md')
      .map(f => {
        const full = path.join(dirPath, f);
        const content = fs.readFileSync(full, 'utf8');
        const titleMatch = content.match(/title:\s*["']?([^"'\r\n]+)["']?/i) || content.match(/^#\s+(.+)$/m);
        const title = titleMatch ? titleMatch[1].replace(/^[📝📅📊🗺️\s]+/, '').trim() : f.replace(/\.md$/, '');
        return {
          fileName: f,
          slug: f.replace(/\.md$/, ''),
          title,
        };
      })
      .sort((a, b) => b.slug.localeCompare(a.slug));
  }

  buildIndex() {
    if (!fs.existsSync(this.digestDir)) {
      return { success: false, count: 0, changed: false, error: 'Digest 目錄不存在' };
    }

    const monthlies = this.scanDir(this.monthlyDir);
    const weeklies = this.scanDir(this.weeklyDir);
    const dailies = this.scanDir(this.dailyDir);

    // 掃描根目錄下的討論串
    const rootFiles = fs.readdirSync(this.digestDir).filter(f => f.endsWith('.md') && f !== 'INDEX.md' && !f.startsWith('WAX_'));
    const digestEntries = [];

    for (const f of rootFiles) {
      const fullPath = path.join(this.digestDir, f);
      const content = fs.readFileSync(fullPath, 'utf8');

      const titleMatch = content.match(/title:\s*["']?Thread:\s*([^"'\r\n]+)["']?/i) || content.match(/^#\s+📝\s+對話脈絡摘要：(.+)$/m) || content.match(/^#\s+📅\s+(.+)$/m);
      const title = titleMatch ? titleMatch[1].trim() : f.replace(/\.md$/, '');
      const slug = f.replace(/\.md$/, '');

      const dateMatch = content.match(/date_range:\s*["']?([^"'\r\n]+)["']?/i);
      const dateRange = dateMatch ? dateMatch[1].trim() : (slug.match(/^\d{4}-\d{2}-\d{2}/) ? slug.slice(0, 10) : '近期');

      const countMatch = content.match(/message_count:\s*(\d+)/i) || content.match(/messages_count:\s*(\d+)/i);
      const msgCount = countMatch ? countMatch[1] : '-';

      const decisions = [];
      const decisionRegex = /- ⭐\s*\*\*([^*]+)\*\*/g;
      let dMatch;
      while ((dMatch = decisionRegex.exec(content)) !== null) {
        decisions.push(dMatch[1].trim());
      }
      const decisionSummary = decisions.length > 0 ? decisions.slice(0, 2).join('；') : '對話脈絡歸檔。';

      digestEntries.push({
        slug,
        title,
        dateRange,
        msgCount,
        decisionSummary,
        fileName: f,
      });
    }

    digestEntries.sort((a, b) => b.slug.localeCompare(a.slug));

    const nowIso = new Date().toISOString();
    const meta = {
      title: '對話脈絡摘要總索引 (Conversation Digest Master Index)',
      type: 'index',
      agent: 'Wax',
      synced_at: nowIso,
      version: 2,
      tags: ['buzz', 'digest', 'index', 'conversations', 'pyramid-hierarchy'],
    };

    let md = buildFrontmatter(meta);
    md += `# 🗂️ Buzz Agent 對話脈絡摘要總索引 (Conversation Digest Master Index)\n\n`;
    md += `> **最後更新時間**：${new Date().toLocaleString('zh-TW', { timeZone: 'Asia/Taipei' })}\n`;
    md += `> **管理代理人**：記憶清潔工 [[Wax]]\n`;
    md += `> **導覽目的**：本索引依「四層知識金字塔（月度里程碑 ➔ 每週複盤 ➔ 每日綜述 ➔ 即時討論串）」維護，為所有 Agent 與主理人提供全景導航地圖。\n\n`;
    md += `---\n\n`;

    // 1. 月度里程碑
    md += `## 🗺️ 月度戰略里程碑 (Monthly Milestones)\n\n`;
    if (monthlies.length > 0) {
      md += `| 月份 | 月度主題與里程碑 (雙向連結) |\n| :--- | :--- |\n`;
      for (const m of monthlies) {
        md += `| \`${m.slug}\` | [[wiki/02-Conversation-Digest/Monthly/${m.slug}\\|🗺️ ${m.title}]] |\n`;
      }
    } else {
      md += `*當前尚未結算月度里程碑*\n`;
    }
    md += `\n`;

    // 2. 每週架構複盤
    md += `## 📊 每週架構與決策複盤 (Weekly Reviews)\n\n`;
    if (weeklies.length > 0) {
      md += `| 週別 | 每週複盤報告 (雙向連結) |\n| :--- | :--- |\n`;
      for (const w of weeklies) {
        md += `| \`${w.slug}\` | [[wiki/02-Conversation-Digest/Weekly/${w.slug}\\|📊 ${w.title}]] |\n`;
      }
    } else {
      md += `*當前尚未結算每週複盤*\n`;
    }
    md += `\n`;

    // 3. 每日對話脈絡
    md += `## 📅 每日對話脈絡綜述 (Daily Summaries)\n\n`;
    if (dailies.length > 0) {
      md += `| 日期 | 每日脈絡綜述 (雙向連結) |\n| :--- | :--- |\n`;
      for (const d of dailies) {
        md += `| \`${d.slug}\` | [[wiki/02-Conversation-Digest/Daily/${d.slug}\\|📅 ${d.title}]] |\n`;
      }
    } else {
      md += `*當前無每日分篇檔案*\n`;
    }
    md += `\n`;

    // 4. 重大歷史討論串總覽
    md += `## 💬 經典對話討論串總覽 (Threads Summary)\n\n`;
    md += `| 討論串主題 (雙向連結) | 對話時間範圍 | 訊息數 | 核心決策與結論提煉 |\n`;
    md += `| :--- | :--- | :--- | :--- |\n`;

    for (const d of digestEntries) {
      md += `| [[${d.slug}\\|${d.title}]] | \`${d.dateRange}\` | ${d.msgCount} 則 | ${d.decisionSummary} |\n`;
    }
    md += `\n---\n\n`;

    md += `## 🔍 決策回溯卡片清單 (Decision Quick Jump)\n\n`;
    for (const d of digestEntries.slice(0, 10)) {
      md += `### 💬 [[${d.slug}]]\n`;
      md += `- **主題**: ${d.title}\n`;
      md += `- **檔案**: \`wiki/02-Conversation-Digest/${d.fileName}\`\n`;
      md += `- **核心決策**: ${d.decisionSummary}\n`;
      md += `- **連結實體**: [[歡迎]], [[core-memory]], [[wiki/01-Cold-Memory/INDEX|Cold Memory Index]]\n\n`;
    }

    md += `> [!NOTE]\n`;
    md += `> 本索引由 Wax 每 30 分鐘背景增量巡檢維護。若主理人在 Obsidian 內部手動修改摘要，系統將遵循「人工編輯最高原則」予以保留。\n`;

    let changed = true;
    if (fs.existsSync(this.indexPath)) {
      const existing = fs.readFileSync(this.indexPath, 'utf8');
      const normalize = (txt) => txt.replace(/最後更新時間.*?\n/, '').replace(/synced_at:\s*"[^"]*"/g, '').trim();
      if (normalize(existing) === normalize(md)) {
        changed = false;
      }
    }

    if (changed) {
      fs.writeFileSync(this.indexPath, md, 'utf8');
    }

    return {
      success: true,
      count: digestEntries.length + dailies.length + weeklies.length + monthlies.length,
      changed,
      indexPath: this.indexPath,
    };
  }
}
