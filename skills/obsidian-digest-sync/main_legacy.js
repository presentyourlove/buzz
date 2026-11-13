/**
 * Obsidian Digest Sync Skill
 * 核心功能：自動化對話摘要生成與 Obsidian Vault 雙向知識庫同步
 */

import fs from 'node:fs';
import path from 'node:path';

export class ObsidianDigestSync {
  constructor(config = {}) {
    this.vaultPath = config.vault_path || 'C:/私人/ai agent/buzz-obsidian';
    this.dailyFolder = config.daily_note_folder || '02-Conversation-Digest/Daily';
    this.tags = config.tags || ['daily-evolution', 'buzz', 'auto-sync'];
  }

  ensureDirectory(dirPath) {
    if (!fs.existsSync(dirPath)) {
      fs.mkdirSync(dirPath, { recursive: true });
    }
  }

  generateDigestMarkdown(dateStr, threadData) {
    const frontmatter = [
      '---',
      `title: "${dateStr} 每日系統進化與決策複盤"`,
      'type: "daily-digest"',
      'agent: "Pollen & Wax"',
      `date: "${dateStr}"`,
      `threads_count: ${threadData.threads_count || 1}`,
      `messages_count: ${threadData.messages_count || 0}`,
      `synced_at: "${new Date().toISOString()}"`,
      `tags: [${this.tags.map(t => JSON.stringify(t)).join(', ')}]`,
      '---',
      ''
    ].join('\n');

    let content = frontmatter;
    content += `# 📅 ${dateStr} 每日系統進化與決策複盤\n\n`;
    content += `> **同步時間**：${new Date().toLocaleString('zh-TW', { timeZone: 'Asia/Taipei' })}  \n`;
    content += `> **執行代理人**：Pollen & Sentry & Wax  \n\n`;
    content += `## 🎯 決策提煉與落地成果\n\n`;
    if (threadData.decisions && threadData.decisions.length > 0) {
      for (const d of threadData.decisions) {
        content += `- ⭐ **${d}**\n`;
      }
    } else {
      content += `- ⭐ **全量系統進化實作通過驗收。**\n`;
    }
    content += `\n## 🔗 雙向連結\n\n- [[core-memory]]\n- [[02-Conversation-Digest/INDEX|對話索引]]\n`;
    return content;
  }

  syncToVault(dateStr, threadData) {
    const targetDir = path.resolve(this.vaultPath, this.dailyFolder);
    this.ensureDirectory(targetDir);
    const filePath = path.join(targetDir, `${dateStr}.md`);
    const md = this.generateDigestMarkdown(dateStr, threadData);
    fs.writeFileSync(filePath, md, 'utf8');
    return { success: true, filePath };
  }
}

export default ObsidianDigestSync;
