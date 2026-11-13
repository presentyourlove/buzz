/**
 * Digest Collector - 從 Buzz 頻道收集指定日期的訊息
 * 使用 buzz CLI 或現有的 harvest 機制獲取訊息
 */

import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';

export class DigestCollector {
  constructor(config = {}) {
    this.channelId = config.channel_id || '5ca019f6-1a27-592a-b015-4baf0eb33572';
    const rawWorkingDir = config.working_dir || 'C:/Users/user/.buzz';
    this.workingDir = this._normalizePath(rawWorkingDir);
    this.buzzCliPath = this._normalizePath(config.buzz_cli_path || 'C:/Users/user/AppData/Local/Buzz/buzz.exe');
    this.relayUrl = config.relay_url || process.env.BUZZ_RELAY_URL || 'https://presentyourlove.communities.buzz.xyz';
    this.privateKey = config.private_key || process.env.BUZZ_PRIVATE_KEY;
    this.authTag = config.auth_tag || process.env.BUZZ_AUTH_TAG;
  }

  /**
   * 路徑正規化 (支援將 /c/... 轉為 C:/...)
   */
  _normalizePath(p) {
    if (!p) return process.cwd();
    if (process.platform === 'win32' && /^\/[a-zA-Z]\//.test(p)) {
      return p[1].toUpperCase() + ':' + p.slice(2);
    }
    return path.normalize(p);
  }

  /**
   * 解析認證憑證 (若環境變數無，自動從 managed-agents.json 取得)
   */
  _resolveCredentials() {
    if (this.privateKey && this.authTag) {
      return { privateKey: this.privateKey, authTag: this.authTag };
    }

    let privateKey = this.privateKey;
    let authTag = this.authTag;

    try {
      const appData = process.env.APPDATA || (process.platform === 'win32' ? 'C:/Users/user/AppData/Roaming' : '');
      const configPath = path.join(appData, 'xyz.block.buzz.app', 'agents', 'managed-agents.json');
      if (fs.existsSync(configPath)) {
        const agents = JSON.parse(fs.readFileSync(configPath, 'utf8'));
        // 優先尋找 Pollen，否則取第一個具備 nsec 的 agent
        const agent = agents.find(a => a.pubkey?.startsWith('6f38f58d')) ||
                      agents.find(a => a.private_key_nsec && a.auth_tag);
        if (agent) {
          if (!privateKey && agent.private_key_nsec) privateKey = agent.private_key_nsec;
          if (!authTag && agent.auth_tag) authTag = agent.auth_tag;
        }
      }
    } catch (e) {
      // 容錯跳過
    }

    return { privateKey, authTag };
  }

  /**
   * 獲取指定日期的頻道訊息
   * @param {string} dateStr - YYYY-MM-DD 格式
   * @returns {Promise<ChannelMessages>} 收集結果
   */
  async collect(dateStr) {
    // 轉換為 Unix timestamp (秒)
    const startOfDay = Math.floor(new Date(`${dateStr}T00:00:00Z`).getTime() / 1000);
    const endOfDay = Math.floor(new Date(`${dateStr}T23:59:59Z`).getTime() / 1000);

    const creds = this._resolveCredentials();

    try {
      const cliArgs = [
        '--relay', this.relayUrl,
        'messages', 'get',
        '--channel', this.channelId,
        '--since', startOfDay.toString(),
        '--before', endOfDay.toString(),
        '--limit', '500'
      ];

      if (creds.authTag) {
        cliArgs.unshift(creds.authTag);
        cliArgs.unshift('--auth-tag');
      }

      // 安全性防禦 (CWE-214)：禁止將私鑰透過 CLI 參數傳遞，僅透過環境變數注入，防止作業系統進程表與例外錯誤洩漏私鑰
      const env = {
        ...process.env,
        BUZZ_RELAY_URL: this.relayUrl
      };
      if (creds.privateKey) env.BUZZ_PRIVATE_KEY = creds.privateKey;
      if (creds.authTag) env.BUZZ_AUTH_TAG = creds.authTag;

      const output = execFileSync(this.buzzCliPath, cliArgs, {
        cwd: this.workingDir,
        encoding: 'utf8',
        maxBuffer: 10 * 1024 * 1024, // 10MB
        env
      });

      const messages = JSON.parse(output);
      
      // 過濾並標準化訊息格式
      const normalizedMessages = messages
        .filter(m => m.content && m.content.trim().length > 0)
        .map(m => ({
          id: m.id,
          author: m.author,
          author_name: m.author_name || m.author,
          content: m.content,
          created_at: m.created_at,
          kind: m.kind,
          tags: m.tags || [],
          mentions: this.extractMentions(m.content, m.tags)
        }));

      const participants = new Set(normalizedMessages.map(m => m.author));
      
      return {
        channel_id: this.channelId,
        date: dateStr,
        messages: normalizedMessages,
        participant_count: participants.size,
        message_count: normalizedMessages.length
      };
    } catch (error) {
      throw new Error(`收集訊息失敗: ${error.message}`);
    }
  }

  /**
   * 從內容和標籤中提取 @mention
   */
  extractMentions(content, tags) {
    const mentions = [];
    // 從標籤提取 p 標籤
    for (const tag of tags) {
      if (tag[0] === 'p' && tag[1]) {
        mentions.push(tag[1]);
      }
    }
    // 從內容中簡單提取 @名稱 (作為補充)
    const mentionRegex = /@(\S+)/g;
    let match;
    while ((match = mentionRegex.exec(content)) !== null) {
      mentions.push(match[1]);
    }
    return [...new Set(mentions)];
  }
}

export default DigestCollector;
