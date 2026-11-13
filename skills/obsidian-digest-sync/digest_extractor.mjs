/**
 * Digest Extractor - 使用 LLM 進行對話萃取：決策、架構、待辦、雜訊過濾
 */

import fs from 'node:fs';
import path from 'node:path';

export class DigestExtractor {
  constructor(config = {}) {
    this.llmProvider = config.llm_provider || process.env.BUZZ_LLM_PROVIDER || 'auto';
    this.apiKey = config.api_key || process.env.OPENROUTER_API_KEY || process.env.OPENAI_API_KEY || process.env.OPENAI_COMPAT_API_KEY;
    this.baseUrl = config.base_url || process.env.OPENROUTER_BASE_URL || process.env.OPENAI_BASE_URL || process.env.OPENAI_COMPAT_BASE_URL;
    this.model = config.model || process.env.OPENROUTER_MODEL || process.env.OPENAI_COMPAT_MODEL;
    this.maxRetries = config.max_retries || 2;
    this._initLLMConfig();
  }

  /**
   * 自動探測與載入全域 LLM 設定
   */
  _initLLMConfig() {
    if (this.apiKey && this.baseUrl && this.model) return;

    try {
      const appData = process.env.APPDATA || (process.platform === 'win32' ? 'C:/Users/user/AppData/Roaming' : '');
      const globalConfigPath = path.join(appData, 'xyz.block.buzz.app', 'agents', 'global-agent-config.json');
      if (fs.existsSync(globalConfigPath)) {
        const globalCfg = JSON.parse(fs.readFileSync(globalConfigPath, 'utf8'));
        const envVars = globalCfg.env_vars || {};
        if (!this.apiKey) {
          this.apiKey = envVars.OPENROUTER_API_KEY || envVars.OPENAI_API_KEY || envVars.OPENAI_COMPAT_API_KEY;
        }
        if (!this.baseUrl && envVars.OPENROUTER_API_KEY) {
          this.baseUrl = 'https://openrouter.ai/api/v1';
        }
        if (!this.model) {
          this.model = globalCfg.model || 'nvidia/nemotron-3-nano-omni-30b-a3b-reasoning:free';
        }
      }
    } catch (e) {
      // 容錯跳過
    }

    if (!this.baseUrl) {
      this.baseUrl = 'https://openrouter.ai/api/v1';
    }
    if (!this.model) {
      this.model = 'nvidia/nemotron-3-nano-omni-30b-a3b-reasoning:free';
    }
  }

  /**
   * 從頻道訊息中萃取結構化摘要
   * @param {ChannelMessages} channelMessages - 收集到的頻道訊息
   * @returns {Promise<DigestExtraction>} 萃取結果
   */
  async extract(channelMessages) {
    const prompt = this.buildPrompt(channelMessages);
    
    for (let attempt = 0; attempt <= this.maxRetries; attempt++) {
      try {
        const response = await this.callLLM(prompt, channelMessages);
        const parsed = this.parseResponse(response);
        return this.validateExtraction(parsed);
      } catch (error) {
        if (attempt === this.maxRetries) {
          console.warn(`[DigestExtractor] 遠端 LLM 調用已達上限 (${error.message})，啟動智慧啟發式降級萃取器...`);
          return this.validateExtraction(this.heuristicExtract(channelMessages));
        }
        // 指數重試
        await new Promise(r => setTimeout(r, 1500 * (attempt + 1)));
      }
    }
  }

  /**
   * 建構 LLM 提示詞
   */
  buildPrompt(channelMessages) {
    const messagesText = channelMessages.messages
      .map(m => `[${new Date(m.created_at * 1000).toISOString().slice(11, 19)}] ${m.author_name}: ${m.content}`)
      .join('\n\n');

    return `# Digest Extractor System Prompt

你是 Buzz 團隊的記憶清潔工 Wax。任務：從頻道對話中萃取「去蕪存菁」的結構化摘要。

## 輸入
- 頻道 ID: ${channelMessages.channel_id}
- 日期: ${channelMessages.date}
- 參與者數: ${channelMessages.participant_count}
- 訊息數: ${channelMessages.message_count}
- 訊息列表:
${messagesText}

## 輸出 (嚴格 JSON，勿包含額外文字或 markdown 程式碼區塊標記)
{
  "summary": "核心摘要 (<=200 字，結論先行)",
  "key_decisions": ["決策1", "決策2"],
  "architecture_consensus": ["架構共識1", "架構共識2"],
  "action_items": [
    {"description": "任務描述", "assignee": "@Agent名", "priority": "High", "due_date": "${channelMessages.date}"}
  ],
  "topics": ["技術主題1", "架構演進", "自動化"]
}

## 萃取規則 (鐵律)
1. **剔除**：客套問候、閒聊廢話、重複發言、純狀態回報 (除非含決策)
2. **保留**：老闆關鍵決策、架構共識、具體待辦落地、阻礙與解決方案
3. **格式**：繁體中文、結構化、雙向連結標註核心實體 \`[[實體名稱]]\`
4. **安全**：偵測到 API Key/Token/私鑰/密碼 -> 替換為 [PROTECTED_SECRET]
5. **標籤**：從 topics 自動生成，小寫連字符 (如 tech-debt, deployment)`;
  }

  /**
   * 調用真實 LLM
   */
  async callLLM(prompt, channelMessages) {
    if (!this.apiKey) {
      throw new Error('未設定 LLM API Key (OPENROUTER_API_KEY / OPENAI_API_KEY)');
    }

    const endpoint = this.baseUrl.replace(/\/+$/, '') + '/chat/completions';
    const controller = new AbortController();
    const timeout = setTimeout(() => controller.abort(), 60000); // 60s timeout

    try {
      const response = await fetch(endpoint, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'Authorization': `Bearer ${this.apiKey}`
        },
        body: JSON.stringify({
          model: this.model,
          messages: [
            {
              role: 'system',
              content: 'You are Wax, a precise meeting summarizer. Always respond with strict valid JSON only, without any Markdown fence or conversational text.'
            },
            {
              role: 'user',
              content: prompt
            }
          ],
          temperature: 0.2
        }),
        signal: controller.signal
      });

      if (!response.ok) {
        const errText = await response.text();
        throw new Error(`LLM HTTP ${response.status}: ${errText.slice(0, 200)}`);
      }

      const data = await response.json();
      const content = data.choices?.[0]?.message?.content;
      if (!content) {
        throw new Error('LLM 回應內容為空');
      }
      return content;
    } finally {
      clearTimeout(timeout);
    }
  }

  /**
   * 智慧啟發式降級萃取器 (當 LLM 配額耗盡或離線時的強韌保底方案)
   */
  heuristicExtract(channelMessages) {
    const messages = channelMessages.messages || [];
    const decisions = [];
    const consensuses = [];
    const actions = [];
    const topicsSet = new Set(['對話記錄', '自動摘要']);

    for (const msg of messages) {
      const text = msg.content || '';
      const author = msg.author_name || msg.author || 'Member';

      if (/決策|決定|同意|通過|確認立項|批准|核准/i.test(text)) {
        decisions.push(`[${author}] ${text.slice(0, 120).replace(/\n+/g, ' ')}`);
      }
      if (/架構|規範|模組|技能|協議|pipeline|daemon|cli/i.test(text)) {
        consensuses.push(`[${author}] ${text.slice(0, 120).replace(/\n+/g, ' ')}`);
      }
      if (/TODO|待辦|請執行|請實作|驗收|修復|修補/i.test(text)) {
        actions.push({
          description: text.slice(0, 100).replace(/\n+/g, ' '),
          assignee: msg.mentions?.[0] ? `@${msg.mentions[0]}` : `@${author}`,
          priority: 'High',
          due_date: channelMessages.date
        });
      }
    }

    if (decisions.length === 0) {
      decisions.push(`${channelMessages.date} 頻道協作與日常技術運作記錄`);
    }
    if (consensuses.length === 0) {
      consensuses.push(`維持各特工既有模組化職責與自動化維運規範`);
    }

    return {
      summary: `本篇為 ${channelMessages.date} 頻道對話自動萃取摘要，本日共記錄 ${messages.length} 則訊息，涵蓋 ${channelMessages.participant_count} 位成員之交流。`,
      key_decisions: decisions.slice(0, 5),
      architecture_consensus: consensuses.slice(0, 5),
      action_items: actions.slice(0, 5),
      topics: Array.from(topicsSet)
    };
  }

  /**
   * 解析 LLM 回應
   */
  parseResponse(response) {
    // 剔除可能的 markdown code fences
    let clean = response.trim();
    if (clean.startsWith('```')) {
      clean = clean.replace(/^```(?:json)?\s*/i, '').replace(/\s*```$/, '');
    }

    // 提取 JSON 部分
    const jsonMatch = clean.match(/\{[\s\S]*\}/);
    if (!jsonMatch) {
      throw new Error('LLM 回應中未找到有效 JSON');
    }
    return JSON.parse(jsonMatch[0]);
  }

  /**
   * 驗證萃取結果
   */
  validateExtraction(extraction) {
    const required = ['summary', 'key_decisions', 'architecture_consensus', 'action_items', 'topics'];
    for (const field of required) {
      if (!(field in extraction)) {
        throw new Error(`萃取結果缺少必要欄位: ${field}`);
      }
    }
    // 確保陣列
    extraction.key_decisions = Array.isArray(extraction.key_decisions) ? extraction.key_decisions : [];
    extraction.architecture_consensus = Array.isArray(extraction.architecture_consensus) ? extraction.architecture_consensus : [];
    extraction.action_items = Array.isArray(extraction.action_items) ? extraction.action_items : [];
    extraction.topics = Array.isArray(extraction.topics) ? extraction.topics : [];
    
    // 安全過濾：替換敏感資訊
    extraction.summary = this.sanitizeSecrets(extraction.summary);
    extraction.key_decisions = extraction.key_decisions.map(d => this.sanitizeSecrets(d));
    extraction.architecture_consensus = extraction.architecture_consensus.map(d => this.sanitizeSecrets(d));
    extraction.action_items = extraction.action_items.map(item => ({
      ...item,
      description: this.sanitizeSecrets(item.description)
    }));
    
    return extraction;
  }

  /**
   * 簡單的敏感資訊過濾
   */
  sanitizeSecrets(text) {
    if (typeof text !== 'string') return '';
    return text
      .replace(/(nsec1[a-z0-9]{20,}|sk-or-v1-[A-Za-z0-9_\-]{16,}|sk-[A-Za-z0-9_\-]{16,}|ghp_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,})/gi, '[PROTECTED_SECRET]')
      .replace(/(sk-|pk-|api[_-]?key|token|secret|password)["'\s:=]+[A-Za-z0-9_\-]{12,}/gi, '[PROTECTED_SECRET]')
      .replace(/[A-Za-z0-9_\-]{40,}/g, '[PROTECTED_SECRET]');
  }
}

export default DigestExtractor;
