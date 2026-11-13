/**
 * Digest Formatter - Frontmatter + Markdown 格式化
 * 產出符合 YYYY-MM-DD_主題.md 命名規範的標準化 Markdown
 */

import { buildFrontmatter } from '../../PLANS/sync_obsidian/obsidian-writer.mjs';

export class DigestFormatter {
  constructor(config = {}) {
    this.outputDir = config.output_dir || 'wiki/02-Conversation-Digest';
    this.defaultTags = config.default_tags || ['digest', 'buzz-memory', 'auto-sync'];
  }

  /**
   * 格式化為完整的 Markdown 檔案內容
   * @param {DigestExtraction} extraction - 萃取結果
   * @param {ChannelMessages} channelMessages - 原始訊息資料
   * @returns {DigestOutput} 格式化輸出
   */
  format(extraction, channelMessages) {
    const dateStr = channelMessages.date;
    const topic = this.generateTopic(extraction);
    const fileName = `${dateStr}_${topic}.md`;
    
    // 建構 Frontmatter
    const frontmatter = {
      title: `${dateStr} ${topic}`,
      type: 'daily-digest',
      agent: 'Pollen & Wax',
      date: dateStr,
      channel_id: channelMessages.channel_id,
      message_count: channelMessages.message_count,
      participant_count: channelMessages.participant_count,
      tags: [...this.defaultTags, ...extraction.topics.map(t => t.toLowerCase().replace(/\s+/g, '-'))],
      status: 'active',
      synced_at: new Date().toISOString(),
      source: `buzz://channel/${channelMessages.channel_id}/${dateStr}`
    };

    // 建構 Markdown 內文
    const content = this.buildContent(extraction, channelMessages);

    // 完整檔案內容
    const frontmatterStr = buildFrontmatter(frontmatter);
    const fullContent = frontmatterStr + content;

    return {
      frontmatter,
      content: fullContent,
      fileName,
      filePath: `${this.outputDir}/${fileName}`,
      topic
    };
  }

  /**
   * 從萃取結果生成主題標題
   */
  generateTopic(extraction) {
    // 優先使用第一個關鍵決策或架構共識
    if (extraction.key_decisions && extraction.key_decisions.length > 0) {
      return this.slugify(extraction.key_decisions[0]).slice(0, 50);
    }
    if (extraction.architecture_consensus && extraction.architecture_consensus.length > 0) {
      return this.slugify(extraction.architecture_consensus[0]).slice(0, 50);
    }
    if (extraction.topics && extraction.topics.length > 0) {
      return this.slugify(extraction.topics.join('-')).slice(0, 50);
    }
    return '每日對話摘要';
  }

  /**
   * 將中文轉為 slug 格式
   */
  slugify(text) {
    return text
      .replace(/[^\w\u4e00-\u9fff]+/g, '-')  // 非字母數字中文替換為 -
      .replace(/-+/g, '-')                     // 多個 - 合併
      .replace(/^-|-$/g, '');                  // 移除首尾 -
  }

  /**
   * 建構 Markdown 內文
   */
  buildContent(extraction, channelMessages) {
    const dateStr = channelMessages.date;
    let md = '';

    // 標題
    md += `# 📅 ${dateStr} ${this.generateTopic(extraction)}\n\n`;
    
    // 元資訊
    md += `> **同步時間**：${new Date().toLocaleString('zh-TW', { timeZone: 'Asia/Taipei' })}  \n`;
    md += `> **頻道**：${channelMessages.channel_id}  \n`;
    md += `> **參與者**：${channelMessages.participant_count} 人  \n`;
    md += `> **訊息數**：${channelMessages.message_count} 則  \n`;
    md += `> **執行代理人**：Pollen & Wax  \n\n`;

    // 核心摘要 (結論先行)
    md += `## 🎯 核心摘要 (結論先行)\n\n`;
    md += `${extraction.summary}\n\n`;

    // 關鍵決策
    if (extraction.key_decisions && extraction.key_decisions.length > 0) {
      md += `## ⭐ 關鍵決策\n\n`;
      for (const decision of extraction.key_decisions) {
        md += `- **${decision}**\n`;
      }
      md += `\n`;
    }

    // 架構共識
    if (extraction.architecture_consensus && extraction.architecture_consensus.length > 0) {
      md += `## 🏗️ 架構共識\n\n`;
      for (const consensus of extraction.architecture_consensus) {
        md += `- **${consensus}**\n`;
      }
      md += `\n`;
    }

    // 待辦落地清單
    if (extraction.action_items && extraction.action_items.length > 0) {
      md += `## ✅ 待辦落地清單\n\n`;
      md += `| 優先級 | 任務 | 負責人 | 截止日期 |\n`;
      md += `| :--- | :--- | :--- | :--- |\n`;
      for (const item of extraction.action_items) {
        const priority = item.priority || 'Medium';
        const priorityIcon = priority === 'High' ? '🔴' : priority === 'Medium' ? '🟡' : '🟢';
        const assignee = item.assignee || '—';
        const dueDate = item.due_date || '—';
        md += `| ${priorityIcon} ${priority} | ${item.description} | ${assignee} | ${dueDate} |\n`;
      }
      md += `\n`;
    }

    // 討論主題標籤
    if (extraction.topics && extraction.topics.length > 0) {
      md += `## 🏷️ 討論主題\n\n`;
      md += extraction.topics.map(t => `\`${t}\``).join('  ') + '\n\n';
    }

    // 雙向連結
    md += `## 🔗 雙向連結\n\n`;
    md += `- [[core-memory]]\n`;
    md += `- [[wiki/02-Conversation-Digest/INDEX|對話索引]]\n`;
    md += `- [[${dateStr}_${this.generateTopic(extraction)}|本日摘要]]\n`;

    return md;
  }
}

export default DigestFormatter;
