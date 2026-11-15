import fs from 'node:fs';
import path from 'node:path';

/**
 * 產生合規的 YAML Frontmatter 字串
 * @param {object} meta - 元資料物件
 * @returns {string} 包含首尾 --- 的 Frontmatter 字串
 */
export function buildFrontmatter(meta = {}) {
  const lines = ['---'];
  for (const [key, value] of Object.entries(meta)) {
    if (value === undefined || value === null) continue;
    if (Array.isArray(value)) {
      const items = value.map((v) => (typeof v === 'string' ? v : JSON.stringify(v))).join(', ');
      lines.push(`${key}: [${items}]`);
    } else if (typeof value === 'object') {
      lines.push(`${key}: ${JSON.stringify(value)}`);
    } else if (typeof value === 'number' || typeof value === 'boolean') {
      lines.push(`${key}: ${value}`);
    } else {
      lines.push(`${key}: "${String(value).replace(/"/g, '\\"')}"`);
    }
  }
  lines.push('---');
  return lines.join('\n') + '\n\n';
}

/**
 * Obsidian 筆記寫入器
 */
export class ObsidianWriter {
  constructor(config = {}) {
    this.vaultPath = config.vault_path || 'c:/私人/ai agent/buzz-obsidian';
    this.structure = config.vault_structure || {
      core_memory: '00-Core-Memory/core-memory.md',
      sync_meta_dir: '00-Core-Memory/sync-meta',
    };
  }

  /**
   * 取得完整的絕對路徑
   * @param {string} relativePath - 相對 Vault 的路徑
   * @returns {string} 絕對路徑
   */
  resolvePath(relativePath) {
    return path.resolve(this.vaultPath, relativePath);
  }

  /**
   * 寫入 Core Memory 至 Obsidian Vault
   * @param {string} bodyContent - 正文內容
   * @param {object} [extraMeta] - 額外元資料
   * @returns {{ success: boolean, targetPath: string, changed: boolean, error?: string }}
   */
  writeCoreMemory(bodyContent, extraMeta = {}) {
    try {
      const targetRelPath = this.structure.core_memory || '00-Core-Memory/core-memory.md';
      const absPath = this.resolvePath(targetRelPath);
      const parentDir = path.dirname(absPath);

      // 確保目標資料夾存在
      if (!fs.existsSync(parentDir)) {
        fs.mkdirSync(parentDir, { recursive: true });
      }

      const nowIso = new Date().toISOString();
      const frontmatterMeta = {
        title: 'Buzz Agent 系統核心記憶 (Core Memory)',
        type: 'core',
        agent: 'Buzz-System',
        source: extraMeta.source || 'buzz://memory/core',
        synced_at: nowIso,
        version: extraMeta.version || 1,
        tags: ['buzz', 'core', 'identity', 'rules'],
      };

      const frontmatterStr = buildFrontmatter(frontmatterMeta);
      const fullNewContent = frontmatterStr + bodyContent.trim() + '\n';

      // 檢查內容是否確實有變動（忽略 synced_at 行）
      let changed = true;
      if (fs.existsSync(absPath)) {
        const existingContent = fs.readFileSync(absPath, 'utf8');
        const normalize = (txt) => txt.replace(/synced_at:\s*"[^"]*"/g, '').trim();
        if (normalize(existingContent) === normalize(fullNewContent)) {
          changed = false;
        }
      }

      if (changed) {
        fs.writeFileSync(absPath, fullNewContent, 'utf8');
      }

      return {
        success: true,
        targetPath: absPath,
        changed,
      };
    } catch (err) {
      return {
        success: false,
        targetPath: '',
        changed: false,
        error: `寫入 Obsidian Vault 失敗: ${err.message}`,
      };
    }
  }
}
