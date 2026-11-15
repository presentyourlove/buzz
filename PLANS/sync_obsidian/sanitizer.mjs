/**
 * 敏感資料與標籤安全過濾模組 (Sanitizer)
 * 負責依據規範檢查黑名單標籤與遮罩敏感憑證
 */

/**
 * 預設機敏憑證遮罩正則清單
 */
const DEFAULT_SECRET_PATTERNS = [
  // API Key, Access Token, Secret, Password
  {
    regex: /(api[_-]?key|access[_-]?token|secret[_-]?key|password|passwd|bearer)["']?\s*[:=]\s*["']?([a-zA-Z0-9._-]{20,})/giu,
    replacement: '$1: "[PROTECTED_SECRET]"',
  },
  // 通用 Secret Key (sk-..., pk-..., api-...)
  {
    regex: /\b(sk|pk|api)[_-][a-zA-Z0-9]{24,}\b/giu,
    replacement: '[PROTECTED_SECRET]',
  },
  // Google API Key (AIzaSy...)
  {
    regex: /\bAIzaSy[a-zA-Z0-9_-]{33}\b/gu,
    replacement: '[PROTECTED_SECRET]',
  },
  // Nostr 私鑰 (nsec1...)
  {
    regex: /\bnsec1[02-9ac-hj-np-z]{58}\b/gu,
    replacement: '[PROTECTED_SECRET]',
  },
  // JWT Token (三段式 base64)
  {
    regex: /\beyJ[A-Za-z0-9_-]{10,}\.eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\b/gu,
    replacement: '[PROTECTED_SECRET]',
  },
];

/**
 * 檢查內容是否包含黑名單標籤
 * @param {string} text - 記憶全文
 * @param {string[]} excludeTags - 排除的標籤清單
 * @returns {{ blocked: boolean, matchedTag?: string }}
 */
export function checkTagExclusion(text, excludeTags = ['private', 'secret', 'tmp', 'nosync', '個人']) {
  if (!text || typeof text !== 'string') {
    return { blocked: false };
  }

  for (const tag of excludeTags) {
    // 支援 #tag 與純 tag 比對
    const cleanTag = tag.replace(/^#/, '');
    const pattern = new RegExp(`(?:^|\\s)#${cleanTag}(?:\\s|$|[，。、！？])`, 'iu');
    if (pattern.test(text)) {
      return { blocked: true, matchedTag: `#${cleanTag}` };
    }
  }

  return { blocked: false };
}

/**
 * 對內容執行機敏資訊遮罩
 * @param {string} text - 原始記憶內容
 * @param {Array<{ pattern: string, replacement: string }>} [customPatterns] - 自訂正則清單
 * @returns {string} 遮罩後的安全文字
 */
export function maskSensitiveData(text, customPatterns) {
  if (!text || typeof text !== 'string') {
    return '';
  }

  // 1. 移除 UTF-8 BOM
  let sanitized = text.replace(/^\uFEFF/, '');

  // 2. 套用預設 patterns
  for (const { regex, replacement } of DEFAULT_SECRET_PATTERNS) {
    sanitized = sanitized.replace(regex, replacement);
  }

  // 3. 套用設定檔中傳入的 pattern (如有)
  if (Array.isArray(customPatterns)) {
    for (const item of customPatterns) {
      if (!item.pattern) continue;
      try {
        const cleanPattern = item.pattern.replace(/^\(\?i\)/, '');
        const flags = item.pattern.startsWith('(?i)') ? 'giu' : 'gu';
        const regex = new RegExp(cleanPattern, flags);
        sanitized = sanitized.replace(regex, item.replacement || '[PROTECTED_SECRET]');
      } catch (err) {
        // 忽略無效正則
      }
    }
  }

  return sanitized;
}

/**
 * 完整過濾入口
 * @param {string} content - 待處理文字
 * @param {object} filterConfig - 設定檔中的 filters 區塊
 * @returns {{ allowed: boolean, content: string, reason?: string }}
 */
export function sanitizeMemory(content, filterConfig = {}) {
  const excludeTags = filterConfig.exclude_tags || ['private', 'secret', 'tmp', 'nosync', '個人'];
  const exclusionResult = checkTagExclusion(content, excludeTags);

  if (exclusionResult.blocked) {
    return {
      allowed: false,
      content: '',
      reason: `包含受保護之黑名單標籤: ${exclusionResult.matchedTag}`,
    };
  }

  const masked = maskSensitiveData(content, filterConfig.secret_patterns);
  return {
    allowed: true,
    content: masked,
  };
}
