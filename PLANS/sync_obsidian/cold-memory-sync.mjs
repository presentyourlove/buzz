import fs from 'node:fs';
import path from 'node:path';
import { sanitizeMemory } from './sanitizer.mjs';
import { buildFrontmatter } from './obsidian-writer.mjs';

/**
 * 預設 PLANS 路徑與 Vault 目標路徑
 */
const DEFAULT_PLANS_DIR = 'C:/Users/user/.buzz/PLANS';
const DEFAULT_VAULT_COLD_DIR = 'c:/私人/ai agent/buzz-obsidian/wiki/01-Cold-Memory';

/**
 * 從 Markdown 文本中提取標題
 * @param {string} content - Markdown 原始內容
 * @param {string} fallback - 回退標題
 * @returns {string} 提取到的標題
 */
function extractTitle(content, fallback) {
  const matchH1 = content.match(/^#\s+(.+)$/m);
  if (matchH1 && matchH1[1]) {
    return matchH1[1].trim();
  }
  const matchFm = content.match(/title:\s*["']?([^"'\r\n]+)["']?/i);
  if (matchFm && matchFm[1]) {
    return matchFm[1].trim();
  }
  return fallback;
}

/**
 * 冷記憶同步模組
 */
export class ColdMemorySync {
  constructor(config = {}) {
    this.vaultPath = config.vault_path || 'c:/私人/ai agent/buzz-obsidian';
    this.coldDir = path.resolve(this.vaultPath, config.vault_structure?.cold_memory_dir || 'wiki/01-Cold-Memory');
    this.plansDir = config.plans_dir || DEFAULT_PLANS_DIR;
    this.filters = config.filters || {};

    this.ensureDir();
  }

  ensureDir() {
    if (!fs.existsSync(this.coldDir)) {
      fs.mkdirSync(this.coldDir, { recursive: true });
    }
  }

  /**
   * 執行 PLANS 目錄掃描並增量同步至 Vault Cold Memory
   * @returns {{ success: boolean, count: number, syncedFiles: string[], changed: boolean }}
   */
  syncColdMemories() {
    this.ensureDir();
    if (!fs.existsSync(this.plansDir)) {
      return { success: false, count: 0, syncedFiles: [], changed: false, error: 'PLANS 目錄不存在' };
    }

    const entries = fs.readdirSync(this.plansDir, { withFileTypes: true });
    const syncedFiles = [];
    let hasAnyChanges = false;

    for (const entry of entries) {
      if (!entry.isFile()) continue;
      if (!entry.name.endsWith('.md') && !entry.name.endsWith('.yaml') && !entry.name.endsWith('.yml')) continue;

      const sourceFilePath = path.join(this.plansDir, entry.name);
      const rawContent = fs.readFileSync(sourceFilePath, 'utf8');

      // 安全過濾
      const sanitizeRes = sanitizeMemory(rawContent, this.filters);
      if (!sanitizeRes.allowed) {
        console.warn(`[cold-sync] 檔案 ${entry.name} 包含黑名單標籤，跳過同步`);
        continue;
      }

      const slug = path.basename(entry.name, path.extname(entry.name));
      const targetFilePath = path.join(this.coldDir, `${slug}.md`);

      const title = extractTitle(rawContent, slug);
      const nowIso = new Date().toISOString();

      const meta = {
        title,
        slug,
        type: 'cold',
        agent: 'Honey',
        source: `buzz://plans/${entry.name}`,
        synced_at: nowIso,
        tags: ['buzz', 'cold', 'architecture', 'active'],
        status: 'active',
      };

      const frontmatter = buildFrontmatter(meta);
      // 清除正文開頭可能已有的 frontmatter，避免重複
      const cleanBody = sanitizeRes.content.replace(/^---\r?\n[\s\S]*?\r?\n---\r?\n/, '').trim();
      const newFileContent = frontmatter + cleanBody + '\n';

      // 檢查是否內容實質改變
      let fileChanged = true;
      if (fs.existsSync(targetFilePath)) {
        const existing = fs.readFileSync(targetFilePath, 'utf8');
        const normalize = (txt) => txt.replace(/synced_at:\s*"[^"]*"/g, '').trim();
        if (normalize(existing) === normalize(newFileContent)) {
          fileChanged = false;
        }
      }

      if (fileChanged) {
        fs.writeFileSync(targetFilePath, newFileContent, 'utf8');
        hasAnyChanges = true;
      }

      syncedFiles.push({
        slug,
        title,
        targetFilePath,
        changed: fileChanged,
      });
    }

    return {
      success: true,
      count: syncedFiles.length,
      syncedFiles,
      changed: hasAnyChanges,
    };
  }
}
