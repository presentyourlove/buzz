/**
 * Obsidian Writer - Vault 寫入與 INDEX 更新
 */

import fs from 'node:fs';
import path from 'node:path';

/**
 * 內建 YAML Frontmatter 產生器（消除外部無效相對路徑依賴）
 */
export function buildFrontmatter(meta = {}) {
  const lines = ['---'];
  for (const [key, val] of Object.entries(meta)) {
    if (val === undefined || val === null) continue;
    if (Array.isArray(val)) {
      lines.push(`${key}:`);
      for (const item of val) {
        lines.push(`  - "${String(item).replace(/"/g, '\\"')}"`);
      }
    } else if (typeof val === 'number' || typeof val === 'boolean') {
      lines.push(`${key}: ${val}`);
    } else {
      lines.push(`${key}: "${String(val).replace(/"/g, '\\"')}"`);
    }
  }
  lines.push('---', '');
  return lines.join('\n');
}

export class ObsidianWriter {
  constructor(config = {}) {
    this.vaultPath = config.vault_path || 'c:/私人/ai agent/buzz-obsidian';
    this.outputDir = config.output_dir || 'wiki/02-Conversation-Digest';
  }

  /**
   * 確保目錄存在
   */
  ensureDirectory(dirPath) {
    if (!fs.existsSync(dirPath)) {
      fs.mkdirSync(dirPath, { recursive: true });
    }
  }

  /**
   * 寫入摘要檔案
   * @param {DigestOutput} digestOutput - 格式化後的輸出
   * @returns {WriteResult} 寫入結果
   */
  writeDigest(digestOutput) {
    try {
      const targetDir = path.resolve(this.vaultPath, this.outputDir);
      this.ensureDirectory(targetDir);
      
      // 路徑安全防禦：強制使用 path.basename 防止 Path Traversal
      const safeFileName = path.basename(String(digestOutput.fileName || 'digest.md'));
      const filePath = path.join(targetDir, safeFileName);
      if (!path.resolve(filePath).startsWith(targetDir)) {
        throw new Error(`偵測到非法路徑穿越嘗試: ${digestOutput.fileName}`);
      }
      
      // 檢查是否已存在且內容相同 (忽略 synced_at)
      let changed = true;
      if (fs.existsSync(filePath)) {
        const existing = fs.readFileSync(filePath, 'utf8');
        const normalize = (txt) => txt
          .replace(/synced_at:\s*"[^"]*"/g, '')
          .replace(/> \*\*同步時間\*\*：.*?\n/, '')
          .trim();
        if (normalize(existing) === normalize(digestOutput.content)) {
          changed = false;
        }
      }
      
      if (changed) {
        fs.writeFileSync(filePath, digestOutput.content, 'utf8');
      }
      
      return {
        success: true,
        filePath,
        changed,
        fileName: digestOutput.fileName
      };
    } catch (error) {
      return {
        success: false,
        filePath: '',
        changed: false,
        error: `寫入 Vault 失敗: ${error.message}`
      };
    }
  }

  /**
   * 更新 INDEX.md
   * @param {Array<DigestOutput>} digests - 所有摘要輸出 (用於重建索引)
   * @returns {IndexResult} 索引更新結果
   */
  updateIndex(digests = []) {
    try {
      const digestDir = path.resolve(this.vaultPath, this.outputDir);
      const indexPath = path.join(digestDir, 'INDEX.md');
      
      // 掃描目錄中所有 .md 檔案 (排除 INDEX.md)
      const allFiles = this.scanDigestDir(digestDir);
      
      // 合併傳入的 digests 和掃描到的檔案
      const entries = this.buildEntries(allFiles, digests);
      
      // 生成 INDEX.md 內容
      const md = this.buildIndexMarkdown(entries);
      
      // 檢查是否有變動
      let changed = true;
      if (fs.existsSync(indexPath)) {
        const existing = fs.readFileSync(indexPath, 'utf8');
        const normalize = (txt) => txt
          .replace(/最後更新時間.*?\n/, '')
          .replace(/synced_at:\s*"[^"]*"/g, '')
          .trim();
        if (normalize(existing) === normalize(md)) {
          changed = false;
        }
      }
      
      if (changed) {
        fs.writeFileSync(indexPath, md, 'utf8');
      }
      
      return {
        success: true,
        indexPath,
        changed,
        count: entries.length
      };
    } catch (error) {
      return {
        success: false,
        indexPath: '',
        changed: false,
        count: 0,
        error: `更新 INDEX.md 失敗: ${error.message}`
      };
    }
  }

  /**
   * 掃描摘要目錄
   */
  scanDigestDir(digestDir) {
    if (!fs.existsSync(digestDir)) return [];
    return fs.readdirSync(digestDir)
      .filter(f => f.endsWith('.md') && f !== 'INDEX.md')
      .map(f => {
        const full = path.join(digestDir, f);
        const content = fs.readFileSync(full, 'utf8');
        const titleMatch = content.match(/title:\s*["']?([^"'\r\n]+)["']?/i) || content.match(/^#\s+(.+)$/m);
        const title = titleMatch ? titleMatch[1].replace(/^[📝📅📊🗺️\s]+/, '').trim() : f.replace(/\.md$/, '');
        const dateMatch = content.match(/date:\s*["']?([^"'\r\n]+)["']?/i);
        const date = dateMatch ? dateMatch[1].trim() : (f.match(/^\d{4}-\d{2}-\d{2}/) ? f.slice(0, 10) : '近期');
        const countMatch = content.match(/message_count:\s*(\d+)/i);
        const msgCount = countMatch ? countMatch[1] : '-';
        return { fileName: f, slug: f.replace(/\.md$/, ''), title, date, msgCount };
      })
      .sort((a, b) => b.slug.localeCompare(a.slug));
  }

  /**
   * 建構索引條目
   */
  buildEntries(allFiles, newDigests) {
    const entries = [...allFiles];
    
    // 加入新產出的摘要 (如果尚未在列表中)
    for (const digest of newDigests) {
      const exists = entries.some(e => e.slug === digest.fileName.replace(/\.md$/, ''));
      if (!exists) {
        entries.unshift({
          fileName: digest.fileName,
          slug: digest.fileName.replace(/\.md$/, ''),
          title: digest.topic || digest.fileName.replace(/\.md$/, ''),
          date: digest.frontmatter?.date || '近期',
          msgCount: digest.frontmatter?.message_count || '-'
        });
      }
    }
    
    return entries.sort((a, b) => b.slug.localeCompare(a.slug));
  }

  /**
   * 建構 INDEX.md Markdown 內容
   */
  buildIndexMarkdown(entries) {
    const nowIso = new Date().toISOString();
    const meta = {
      title: '對話脈絡摘要總索引 (Conversation Digest Master Index)',
      type: 'index',
      agent: 'Wax',
      synced_at: nowIso,
      version: 2,
      tags: ['buzz', 'digest', 'index', 'conversations', 'daily-evolution']
    };

    let md = buildFrontmatter(meta);
    md += `# 🗂️ Buzz Agent 對話脈絡摘要總索引 (Conversation Digest Master Index)\n\n`;
    md += `> **最後更新時間**：${new Date().toLocaleString('zh-TW', { timeZone: 'Asia/Taipei' })}\n`;
    md += `> **管理代理人**：記憶清潔工 [[Wax]]\n`;
    md += `> **導覽目的**：本索引依「每日對話脈絡綜述」維護，為所有 Agent 與主理人提供全景導航地圖。\n\n`;
    md += `---\n\n`;

    // 每日對話脈絡
    md += `## 📅 每日對話脈絡綜述 (Daily Summaries)\n\n`;
    if (entries.length > 0) {
      md += `| 日期 | 每日脈絡綜述 (雙向連結) | 訊息數 |\n`;
      md += `| :--- | :--- | :--- |\n`;
      for (const e of entries) {
        md += `| \`${e.date}\` | [[${e.slug}|${e.title}]] | ${e.msgCount} 則 |\n`;
      }
    } else {
      md += `*當前無每日分篇檔案*\n`;
    }
    md += `\n---\n\n`;

    // 決策回溯卡片清單 (前 10 筆)
    md += `## 🔍 決策回溯卡片清單 (Decision Quick Jump)\n\n`;
    for (const e of entries.slice(0, 10)) {
      md += `### 💬 [[${e.slug}]]\n`;
      md += `- **主題**: ${e.title}\n`;
      md += `- **檔案**: \`wiki/02-Conversation-Digest/${e.fileName}\`\n`;
      md += `- **日期**: ${e.date}\n`;
      md += `- **連結實體**: [[歡迎]], [[core-memory]], [[wiki/01-Cold-Memory/INDEX|Cold Memory Index]]\n\n`;
    }

    md += `> [!NOTE]\n`;
    md += `> 本索引由 Wax 定期維護。若主理人在 Obsidian 內部手動修改摘要，系統將遵循「人工編輯最高原則」予以保留。\n`;

    return md;
  }
}

export default ObsidianWriter;
