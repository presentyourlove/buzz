import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

/**
 * 預設設定物件，當 YAML 缺少欄位時作為後備回退
 */
const DEFAULT_CONFIG = {
  vault_path: 'c:/私人/ai agent/buzz-obsidian',
  buzz: {
    relay_url: 'wss://presentyourlove.communities.buzz.xyz',
    cli_path: 'C:/Users/user/AppData/Local/Buzz/buzz.exe',
  },
  sync_intervals: {
    core_memory: 300,
    cold_memory: 600,
    conversation_digest: 1800,
  },
  git: {
    auto_commit: true,
    auto_push: true,
    remote: 'origin',
    branch: 'main',
    init_if_missing: true,
  },
  filters: {
    exclude_channels: [],
    exclude_tags: ['private', 'secret', 'tmp', 'nosync', '個人'],
    secret_patterns: [
      {
        pattern: '(?i)(api[_-]?key|access[_-]?token|secret[_-]?key|password|passwd|bearer)[\"\\\']?\\s*[:=]\\s*[\"\\\']?([a-zA-Z0-9._-]{20,})',
        replacement: '[PROTECTED_SECRET]',
      },
      {
        pattern: '(?i)(sk|pk|api)[_-]?[a-zA-Z0-9]{32,}',
        replacement: '[PROTECTED_SECRET]',
      },
      {
        pattern: 'eyJ[A-Za-z0-9_-]*\\.[A-Za-z0-9_-]*\\.[A-Za-z0-9_-]*',
        replacement: '[PROTECTED_SECRET]',
      },
      {
        pattern: 'nsec1[0-9a-z]{58}',
        replacement: '[PROTECTED_SECRET]',
      },
    ],
  },
  vault_structure: {
    core_memory: 'wiki/00-Core-Memory/core-memory.md',
    cold_memory_dir: 'wiki/01-Cold-Memory',
    cold_memory_index: 'wiki/01-Cold-Memory/INDEX.md',
    conversation_digest_dir: 'wiki/02-Conversation-Digest',
    conversation_digest_index: 'wiki/02-Conversation-Digest/INDEX.md',
    entities_dir: 'wiki/03-Entities',
    sync_meta_dir: 'wiki/00-Core-Memory/sync-meta',
  },
  state_store: {
    path: 'sync-meta/state.json',
    conflict_log: 'sync-meta/conflict-log.json',
    last_sync: 'sync-meta/last-sync.json',
  },
  health_check: {
    enabled: true,
    heartbeat_file: 'sync-meta/heartbeat.json',
    max_stale_seconds: 600,
  },
};

/**
 * 簡易而強健的 YAML 解析器（免除第三方 npm 套件依賴）
 * 針對 config.yaml 的結構進行安全剖析
 *
 * @param {string} yamlText - YAML 原始字串
 * @returns {object} 解析後的 JavaScript 物件
 */
export function parseSimpleYaml(yamlText) {
  const result = {};
  const lines = yamlText.split(/\r?\n/);
  const stack = [{ indent: -1, obj: result }];
  let currentKey = null;

  for (let rawLine of lines) {
    // 移除行內註解，但需保護引號內的內容
    const commentIndex = rawLine.indexOf('#');
    let line = commentIndex >= 0 ? rawLine.slice(0, commentIndex) : rawLine;
    if (!line.trim()) continue;

    const indent = line.search(/\S/);
    const trimmed = line.trim();
    if (trimmed === '---') continue;

    // 回退縮排堆疊
    while (stack.length > 1 && indent <= stack[stack.length - 1].indent) {
      stack.pop();
    }
    let topFrame = stack[stack.length - 1];

    // 清單項目: - value 或 - key: value
    if (trimmed.startsWith('- ') || trimmed === '-') {
      const itemContent = trimmed.slice(1).trim();

      // 若當前容器為空物件，但收到清單項目，將其轉換為陣列
      if (!Array.isArray(topFrame.obj)) {
        if (Object.keys(topFrame.obj).length === 0 && stack.length > 1) {
          const parentFrame = stack[stack.length - 2];
          // 找到 parent 中的鍵並替換為陣列
          for (const [k, v] of Object.entries(parentFrame.obj)) {
            if (v === topFrame.obj) {
              parentFrame.obj[k] = [];
              topFrame.obj = parentFrame.obj;
              break;
            }
          }
        }
      }

      const targetArray = Array.isArray(topFrame.obj) ? topFrame.obj : (topFrame.obj = []);

      if (itemContent.includes(':')) {
        const colonIdx = itemContent.indexOf(':');
        const k = itemContent.slice(0, colonIdx).trim();
        const v = parseScalarValue(itemContent.slice(colonIdx + 1).trim());
        const subObj = { [k]: v };
        targetArray.push(subObj);
        stack.push({ indent, obj: subObj });
      } else {
        targetArray.push(parseScalarValue(itemContent));
      }
      continue;
    }

    // 鍵值對: key: value 或 key:
    if (trimmed.includes(':')) {
      const colonIdx = trimmed.indexOf(':');
      const key = trimmed.slice(0, colonIdx).trim();
      const valStr = trimmed.slice(colonIdx + 1).trim();

      currentKey = key;

      if (valStr === '' || valStr === '[]') {
        const newObj = valStr === '[]' ? [] : {};
        topFrame.obj[key] = newObj;
        stack.push({ indent, obj: newObj });
      } else {
        topFrame.obj[key] = parseScalarValue(valStr);
      }
    }
  }

  return result;
}

/**
 * 將 YAML 純量字串解析為對應的 JS 型別
 * @param {string} val - 純量字串
 * @returns {any} 布林、數字或字串
 */
function parseScalarValue(val) {
  if (val === 'true') return true;
  if (val === 'false') return false;
  if (val === 'null' || val === '~') return null;
  if (val === '[]') return [];
  if (!isNaN(val) && val !== '') return Number(val);
  // 移除首尾雙引號或單引號
  if ((val.startsWith('"') && val.endsWith('"')) || (val.startsWith("'") && val.endsWith("'"))) {
    return val.slice(1, -1);
  }
  return val;
}

/**
 * 載入並融合設定檔
 * @param {string} [customPath] - 自訂 config.yaml 路徑
 * @returns {object} 完整設定物件
 */
export function loadConfig(customPath) {
  const configPath = customPath || path.resolve(__dirname, 'config.yaml');
  if (!fs.existsSync(configPath)) {
    return DEFAULT_CONFIG;
  }

  try {
    const rawContent = fs.readFileSync(configPath, 'utf8');
    const parsed = parseSimpleYaml(rawContent);

    // 深度融合預設值
    return {
      ...DEFAULT_CONFIG,
      ...parsed,
      buzz: { ...DEFAULT_CONFIG.buzz, ...(parsed.buzz || {}) },
      sync_intervals: { ...DEFAULT_CONFIG.sync_intervals, ...(parsed.sync_intervals || {}) },
      git: { ...DEFAULT_CONFIG.git, ...(parsed.git || {}) },
      filters: { ...DEFAULT_CONFIG.filters, ...(parsed.filters || {}) },
      vault_structure: { ...DEFAULT_CONFIG.vault_structure, ...(parsed.vault_structure || {}) },
      state_store: { ...DEFAULT_CONFIG.state_store, ...(parsed.state_store || {}) },
      health_check: { ...DEFAULT_CONFIG.health_check, ...(parsed.health_check || {}) },
    };
  } catch (error) {
    console.warn(`[config-loader] 讀取設定檔失敗，使用預設值: ${error.message}`);
    return DEFAULT_CONFIG;
  }
}
