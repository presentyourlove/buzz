import fs from 'node:fs';
import path from 'node:path';
import { buildFrontmatter } from './obsidian-writer.mjs';

/**
 * 簡易提取純文字摘要
 * @param {string} content - Markdown 內容
 * @returns {string} 單行摘要
 */
function extractSummary(content) {
  // 移除 Frontmatter 與 H1
  const body = content.replace(/^---\r?\n[\s\S]*?\r?\n---\r?\n/, '').replace(/^#\s+.+$/m, '').trim();
  const paragraphs = body.split(/\r?\n\r?\n/);
  for (const p of paragraphs) {
    const clean = p.replace(/[#*`>[\]]/g, '').trim();
    if (clean.length > 15) {
      return clean.length > 80 ? clean.slice(0, 80) + '...' : clean;
    }
  }
  return '技術設計與規格說明文件。';
}

/**
 * 冷記憶總索引生成器
 */
export class ColdIndexBuilder {
  constructor(config = {}) {
    this.vaultPath = config.vault_path || 'c:/私人/ai agent/buzz-obsidian';
    this.coldDir = path.resolve(this.vaultPath, config.vault_structure?.cold_memory_dir || 'wiki/01-Cold-Memory');
    this.indexPath = path.resolve(this.coldDir, 'INDEX.md');
  }

  /**
   * 掃描並重建 INDEX.md
   * @returns {{ success: boolean, count: number, changed: boolean }}
   */
  buildIndex() {
    if (!fs.existsSync(this.coldDir)) {
      return { success: false, count: 0, changed: false, error: 'Cold Memory 目錄不存在' };
    }

    const files = fs.readdirSync(this.coldDir);
    const docEntries = [];

    for (const f of files) {
      if (!f.endsWith('.md') || f === 'INDEX.md') continue;
      const fullPath = path.join(this.coldDir, f);
      const content = fs.readFileSync(fullPath, 'utf8');

      const titleMatch = content.match(/title:\s*["']?([^"'\r\n]+)["']?/i) || content.match(/^#\s+(.+)$/m);
      const title = titleMatch ? titleMatch[1].trim() : f.replace(/\.md$/, '');
      const slug = f.replace(/\.md$/, '');
      const summary = extractSummary(content);

      docEntries.push({
        slug,
        title,
        summary,
        fileName: f,
      });
    }

    // 依檔名字母排序
    docEntries.sort((a, b) => a.slug.localeCompare(b.slug));

    const nowIso = new Date().toISOString();
    const meta = {
      title: '冷記憶知識庫總索引 (Cold Memory Index)',
      type: 'index',
      agent: 'Buzz-System',
      synced_at: nowIso,
      version: 1,
      tags: ['buzz', 'cold', 'index', 'architecture'],
    };

    const frontmatter = buildFrontmatter(meta);

    let md = frontmatter;
    md += `# 📚 Buzz Agent 冷記憶知識庫總索引 (Cold Memory Master Index)\n\n`;
    md += `> **最後索引時間**：${nowIso}\n`;
    md += `> **導覽說明**：本索引匯總由團隊（秘書 [[Fizz]]、雜工 [[Honey]]、工程師 [[Pollen]]、清潔工 [[Wax]]）沉澱之長期架構規格與技術計畫，供 Agent 調研時直接讀取。\n\n`;

    md += `## 📑 技術規格清單總覽 (Technical Plans Overview)\n\n`;
    md += `| 文件代號 (雙向連結) | 文件標題 | 核心摘要 | 狀態 |\n`;
    md += `| :--- | :--- | :--- | :--- |\n`;

    for (const doc of docEntries) {
      md += `| [[${doc.slug}]] | **${doc.title}** | ${doc.summary} | \`生效中 (Active)\` |\n`;
    }

    md += `\n---\n\n`;
    md += `## 🗺️ 架構核心卡片與導覽 (Architecture Entities)\n\n`;

    for (const doc of docEntries) {
      md += `### 📌 [[${doc.slug}]]\n`;
      md += `- **標題**: ${doc.title}\n`;
      md += `- **實體路徑**: \`wiki/01-Cold-Memory/${doc.fileName}\`\n`;
      md += `- **提煉重點**: ${doc.summary}\n`;
      md += `- **相關實體**: [[Buzz架構]], [[Obsidian整合]], [[系統規則]]\n\n`;
    }

    md += `> [!TIP]\n`;
    md += `> Agent 在執行大型開發或調查前，請務必先透過 \`read_file\` 閱讀相關冷記憶文件，嚴禁未掌握規格前盲目表態。\n`;

    // 檢查異動
    let changed = true;
    if (fs.existsSync(this.indexPath)) {
      const existing = fs.readFileSync(this.indexPath, 'utf8');
      const normalize = (txt) => txt.replace(/最後索引時間.*?\n/, '').replace(/synced_at:\s*"[^"]*"/g, '').trim();
      if (normalize(existing) === normalize(md)) {
        changed = false;
      }
    }

    if (changed) {
      fs.writeFileSync(this.indexPath, md, 'utf8');
    }

    return {
      success: true,
      count: docEntries.length,
      changed,
      indexPath: this.indexPath,
    };
  }
}
