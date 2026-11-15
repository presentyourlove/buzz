import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { DatabaseSync } from 'node:sqlite';

/**
 * 預設路徑常數
 */
const BUZZ_EXE_PATH = 'C:/Users/user/AppData/Local/Buzz/buzz.exe';
const MANAGED_AGENTS_PATH = 'C:/Users/user/AppData/Roaming/xyz.block.buzz.app/agents/managed-agents.json';
const ARCHIVE_DB_PATH = 'C:/Users/user/.buzz/archive/archive.db';

/**
 * Buzz 用戶端封裝模組
 * 負責透過 CLI、本機 Agent Registry 或本地 SQLite Archive 提取記憶資料
 */
export class BuzzClient {
  constructor(config = {}) {
    this.config = config;
    this.cliPath = config.buzz?.cli_path || BUZZ_EXE_PATH;
    this.relayUrl = config.buzz?.relay_url || 'wss://presentyourlove.communities.buzz.xyz';
    this.agentsRegistryPath = MANAGED_AGENTS_PATH;
    this.archiveDbPath = ARCHIVE_DB_PATH;
  }

  /**
   * 嘗試透過 Buzz CLI 讀取 Core Memory
   * @returns {{ success: boolean, content?: string, error?: string }}
   */
  fetchCoreFromCli() {
    if (!fs.existsSync(this.cliPath)) {
      return { success: false, error: `Buzz CLI 未在預設路徑找到: ${this.cliPath}` };
    }

    try {
      const result = spawnSync(this.cliPath, ['mem', 'get', 'core', '--relay', this.relayUrl], {
        encoding: 'utf8',
        timeout: 5000,
        windowsHide: true,
        env: { ...process.env, BUZZ_RELAY_URL: this.relayUrl },
      });

      if (result.status === 0 && result.stdout && result.stdout.trim().length > 0) {
        return {
          success: true,
          content: result.stdout.trim(),
        };
      }

      const errorMsg = result.stderr || result.stdout || `Exit code ${result.status}`;
      return { success: false, error: errorMsg };
    } catch (err) {
      return { success: false, error: err.message };
    }
  }

  /**
   * 透過本機 Agent Registry 聚合四大代理人核心設定與系統規則
   * 作為無外部 Relay 憑證時的高強健性本機資料來源
   * @returns {{ success: boolean, content: string, metadata: object }}
   */
  fetchCoreFromLocalRegistry() {
    if (!fs.existsSync(this.agentsRegistryPath)) {
      return { success: false, content: '', error: '找不到 managed-agents.json' };
    }

    try {
      const raw = fs.readFileSync(this.agentsRegistryPath, 'utf8');
      const agentsData = JSON.parse(raw);

      // 去重並抽取核心代理人
      const uniqueAgents = new Map();
      for (const item of Object.values(agentsData)) {
        if (!item || !item.name) continue;
        // 優先保留具有 pubkey 的完整定義
        if (!uniqueAgents.has(item.name) || (!uniqueAgents.get(item.name).pubkey && item.pubkey)) {
          uniqueAgents.set(item.name, item);
        }
      }

      const agentsList = Array.from(uniqueAgents.values());
      const nowIso = new Date().toISOString();

      let markdown = `# 🧠 Buzz Agent 系統核心記憶 (Core Memory)\n\n`;
      markdown += `> **資料權威來源**：Buzz 本機託管代理人核心註冊表 (Local Managed Agents Registry)\n`;
      markdown += `> **版本狀態**：正式生效中 (Active)\n\n`;

      markdown += `## 📌 系統全域原則與鐵律 (Core Principles)\n\n`;
      markdown += `1. **語言限制**：所有對話、規格規劃與程式產出必須使用 **繁體中文 (Traditional Chinese)**。\n`;
      markdown += `2. **代碼落地準則**：代碼實作必須端到端落檔，禁止口頭承諾或空泛回報，必須產出可追溯之 Git Patch 補丁與日誌。\n`;
      markdown += `3. **雙向記憶圖譜**：所有實體與重要概念均主動連結為 \`[[WikiLink]]\`，沉澱於 Obsidian 第二大腦。\n`;
      markdown += `## 🧭 第二大腦導覽與記憶索引 (Second Brain Navigation Radar)\n\n`;
      markdown += `所有 Agent 在被喚醒執行任務時，請遵循以下閱讀指引以獲取最新沉澱知識：\n`;
      markdown += `- 📚 **技術架構與專案規格**：請調用工具讀取 \`[[wiki/01-Cold-Memory/INDEX|01-Cold-Memory/INDEX.md]]\`（包含所有冷記憶與規劃進度）。\n`;
      markdown += `- 🗂️ **歷史對話決策與共識**：請調用工具讀取 \`[[wiki/02-Conversation-Digest/INDEX|02-Conversation-Digest/INDEX.md]]\`（包含所有歷史討論串之精確結論與待辦）。\n`;
      markdown += `- 📜 **總體守則與格式標準**：請隨時參照 \`[[歡迎|歡迎.md]]\`。\n\n`;

      markdown += `## 👥 核心 Agent 團隊陣容與職責邊界\n\n`;

      for (const agent of agentsList) {
        markdown += `### 🤖 ${agent.name} (${agent.display_name || agent.name})\n`;
        markdown += `- **公鑰 (Pubkey)**: \`${agent.pubkey || '未綁定'}\`\n`;
        markdown += `- **模型 (Model)**: \`${agent.model || '未指定'}\` (${agent.provider || 'default'})\n`;
        markdown += `- **觸發限制**: \`${agent.respond_to || 'mentions'}\`\n`;

        if (agent.system_prompt) {
          markdown += `\n**系統提示詞與核心指令**：\n\`\`\`text\n${agent.system_prompt.trim()}\n\`\`\`\n\n`;
        } else {
          markdown += `\n*尚無個別 system prompt*\n\n`;
        }
      }

      return {
        success: true,
        content: markdown,
        metadata: {
          agentCount: agentsList.length,
          agents: agentsList.map((a) => ({ name: a.name, pubkey: a.pubkey })),
        },
      };
    } catch (err) {
      return { success: false, content: '', error: `解析代理人註冊表失敗: ${err.message}` };
    }
  }

  /**
   * 統一取得 Core Memory 入口
   * 優先 CLI，失敗則平滑切換至本機註冊表
   * @returns {{ success: boolean, source: string, content: string, metadata?: object }}
   */
  getCoreMemory() {
    const cliResult = this.fetchCoreFromCli();
    if (cliResult.success) {
      return {
        success: true,
        source: 'buzz_cli',
        content: cliResult.content,
      };
    }

    // 容錯回退至本機註冊表
    const localResult = this.fetchCoreFromLocalRegistry();
    if (localResult.success) {
      return {
        success: true,
        source: 'local_registry_fallback',
        content: localResult.content,
        metadata: localResult.metadata,
      };
    }

    return {
      success: false,
      source: 'none',
      content: '',
      error: `無法自 CLI 或本機資料庫取得記憶: CLI (${cliResult.error}) | Local (${localResult.error})`,
    };
  }

  /**
   * 透過 Wax 身份憑證調用 Buzz CLI 讀取頻道最新即時滾動訊息
   * @param {string} [channelId] - 頻道 ID
   * @param {number} [limit=50] - 抓取筆數
   * @param {number} [since] - 時間戳下限
   * @returns {{ success: boolean, messages: Array<object>, error?: string }}
   */
  fetchLiveMessages(channelId = null, limit = 50, since = null) {
    if (!fs.existsSync(this.cliPath)) {
      return { success: false, messages: [], error: `Buzz CLI 未找到: ${this.cliPath}` };
    }

    const targetChannel = channelId || this.config.wax_auth?.channel_id || '5ca019f6-1a27-592a-b015-4baf0eb33572';
    const privKey = this.config.wax_auth?.private_key || process.env.BUZZ_PRIVATE_KEY || 'nsec1ly388ent2cvr7f9rscxffxgmvuljggxx374sjkfvwee4zrl3p89sqfjmce';
    const authTag = this.config.wax_auth?.auth_tag || process.env.BUZZ_AUTH_TAG || '["auth","2d29f28fd5106795c630120d7e5b062bc4f5caa8057b4a6a374b7468cdc5de8e","","6f67f68423fe1877d312046cc63cf0106b7f31f284eb51d586dc036a235aeb8433d159d97c37e2b927b40fdc68eff0e1071fdaf56c497494d1779a8bc7fc6118"]';

    const env = {
      ...process.env,
      BUZZ_RELAY_URL: this.relayUrl,
      BUZZ_PRIVATE_KEY: privKey,
      BUZZ_AUTH_TAG: authTag,
    };

    const args = ['messages', 'get', '--channel', targetChannel, '--limit', String(limit)];
    if (since) {
      args.push('--since', String(since));
    }

    try {
      const res = spawnSync(this.cliPath, args, { env, encoding: 'utf8', timeout: 10000, windowsHide: true });
      if (res.status === 0 && res.stdout) {
        const msgs = JSON.parse(res.stdout);
        const SENDER_MAP = {
          '2d29f28fd5106795c630120d7e5b062bc4f5caa8057b4a6a374b7468cdc5de8e': '可愛小寶寶 (老闆)',
          '82fdf490a5dce0822cd3c0cba0d77150776f8c4d70b94b411195b1e486b13b26': 'Fizz (秘書)',
          '59e45d75af783cdb01c2cbe0e479db8340949c126921693812c936e760fd7a91': 'Honey (雜工)',
          '247200c9e2e49c4e678c3fe5204c9997ee19b4e58284cd9484892f72d6bcff86': 'Pollen (技術工)',
          '232bd911e39f611c97001f7e79cc2ac2dd55e1635b8d33ea9a6d293872727c1d': 'Wax (記憶清潔官)',
        };

        const enriched = msgs.map(m => ({
          ...m,
          author_name: SENDER_MAP[m.pubkey] || `User_${m.pubkey.slice(0, 6)}`,
          is_owner: m.pubkey === '2d29f28fd5106795c630120d7e5b062bc4f5caa8057b4a6a374b7468cdc5de8e',
        }));

        return { success: true, messages: enriched };
      }
      return { success: false, messages: [], error: res.stderr || res.stdout || `Exit ${res.status}` };
    } catch (err) {
      return { success: false, messages: [], error: err.message };
    }
  }
}
