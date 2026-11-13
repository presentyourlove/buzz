/**
 * Digest Daemon - 常駐排程守護進程
 * 使用 node-cron 定時觸發 obsidian-digest-sync
 * 支援指數退避重試機制
 */

import cron from 'node-cron';
import { DigestSync } from './digest_sync.mjs';

/**
 * 指數退避重試包裝器
 * @param {Function} fn - 要執行的異步函數
 * @param {Object} options - 重試選項
 * @param {number} options.maxRetries - 最大重試次數 (預設 3)
 * @param {number} options.baseDelay - 基礎延遲毫秒數 (預設 30000 = 30秒)
 * @param {string} options.taskName - 任務名稱 (用於日誌)
 * @returns {Promise<any>} 執行結果
 */
async function withRetry(fn, options = {}) {
  const {
    maxRetries = 3,
    baseDelay = 30000, // 30秒
    taskName = 'Task'
  } = options;

  let lastError;
  
  for (let attempt = 0; attempt <= maxRetries; attempt++) {
    try {
      if (attempt > 0) {
        console.log(`[${taskName}] 第 ${attempt} 次重試...`);
      }
      return await fn();
    } catch (error) {
      lastError = error;
      if (attempt < maxRetries) {
        const delay = baseDelay * Math.pow(2, attempt); // 指數退避: 30s, 60s, 120s
        console.log(`[${taskName}] 失敗 (${error.message})，${delay/1000}秒後重試 (${attempt + 1}/${maxRetries})`);
        await new Promise(resolve => setTimeout(resolve, delay));
      }
    }
  }
  
  throw new Error(`[${taskName}] 重試 ${maxRetries} 次後仍失敗: ${lastError.message}`);
}

/**
 * 執行每日收割任務 (含重試)
 */
async function runDailyDigest(digestSync) {
  return await withRetry(
    async () => {
      const result = await digestSync.runOnce(null); // null = 昨天
      if (!result.success) {
        throw new Error(result.error || '收割失敗');
      }
      return result;
    },
    {
      maxRetries: 3,
      baseDelay: 30000,
      taskName: 'DailyDigest'
    }
  );
}

/**
 * 建立並啟動 Daemon
 */
export function createDaemon(config = {}) {
  // 解析 Cron 排程 (環境變數優先，其次預設值)
  const cronSchedule = process.env.CRON_SCHEDULE || '0 2 * * *'; // 每天 02:00
  
  // 初始化 DigestSync
  const digestSync = new DigestSync(config);
  
  console.log(`[Digest Daemon] 啟動中...`);
  console.log(`[Digest Daemon] Cron 排程: ${cronSchedule}`);
  console.log(`[Digest Daemon] 目標頻道: ${digestSync.config.channel_id}`);
  console.log(`[Digest Daemon] Vault 路徑: ${digestSync.config.vault_path}`);
  
  // 驗證 Cron 表達式
  if (!cron.validate(cronSchedule)) {
    throw new Error(`無效的 Cron 表達式: ${cronSchedule}`);
  }
  
  // 建立排程任務
  const task = cron.schedule(cronSchedule, async () => {
    const now = new Date().toISOString();
    console.log(`\n[${now}] ⏰ Cron 觸發：開始每日收割任務`);
    
    try {
      const result = await runDailyDigest(digestSync);
      console.log(`[${new Date().toISOString()}] ✅ 每日收割完成: ${result.message}`);
    } catch (error) {
      console.error(`[${new Date().toISOString()}] ❌ 每日收割最終失敗: ${error.message}`);
      // 這裡可以加入發送失敗通知到頻道的邏輯
    }
  }, {
    scheduled: false, // 不自動啟動，由呼叫者控制
    timezone: 'Asia/Taipei' // 使用台灣時區
  });
  
  return {
    task,
    digestSync,
    cronSchedule,
    
    /**
     * 啟動 Daemon
     */
    start() {
      console.log(`[Digest Daemon] 守護進程已啟動，等待排程觸發...`);
      task.start();
    },
    
    /**
     * 停止 Daemon
     */
    stop() {
      console.log(`[Digest Daemon] 正在停止守護進程...`);
      task.stop();
      console.log(`[Digest Daemon] 守護進程已停止`);
    },
    
    /**
     * 立即執行一次 (手動觸發)
     */
    async runNow() {
      console.log(`[Digest Daemon] 手動觸發單次收割...`);
      return await runDailyDigest(digestSync);
    },
    
    /**
     * 取得下次執行時間
     */
    getNextRun() {
      try {
        const nextDates = cron.schedule(cronSchedule, () => {}, { 
          scheduled: false,
          timezone: 'Asia/Taipei'
        });
        // node-cron 不直接提供下次執行時間，這裡簡單計算
        return '根據 Cron 表達式計算';
      } catch {
        return '無法計算';
      }
    }
  };
}

/**
 * CLI 入口點 - 直接執行時啟動 Daemon
 */
async function main() {
  const config = {
    channel_id: process.env.BUZZ_DIGEST_SYNC_CHANNEL || '5ca019f6-1a27-592a-b015-4baf0eb33572',
    vault_path: process.env.BUZZ_OBSIDIAN_VAULT_PATH || 'c:/私人/ai agent/buzz-obsidian',
    output_dir: process.env.BUZZ_DIGEST_OUTPUT_DIR || 'wiki/02-Conversation-Digest',
    sync_command: process.env.BUZZ_SYNC_OBSIDIAN_CMD || 'node PLANS/sync_obsidian/main.mjs',
    working_dir: process.env.BUZZ_SYNC_WORKING_DIR || '/c/Users/user/.buzz'
  };
  
  // 檢查是否為測試模式 (短週期 cron)
  const isTestMode = process.env.CRON_SCHEDULE === '* * * * *';
  
  if (isTestMode) {
    console.log(`[Digest Daemon] 🧪 測試模式：每分鐘執行一次`);
  }
  
  const daemon = createDaemon(config);
  
  // 優雅關閉處理
  const shutdown = () => {
    console.log(`\n[Digest Daemon] 接收到關閉信號，正在優雅關閉...`);
    daemon.stop();
    process.exit(0);
  };
  
  process.on('SIGINT', shutdown);
  process.on('SIGTERM', shutdown);
  
  // 啟動
  daemon.start();
  
  // 測試模式：額外提供手動觸發提示
  if (isTestMode) {
    console.log(`[Digest Daemon] 測試模式提示：按 Ctrl+C 停止，或等待下一分鐘自動觸發`);
  }
  
  // 保持進程存活
  await new Promise(() => {}); // 永不解析，保持運行
}

// 只有直接執行時才啟動
if (import.meta.url === `file://${process.argv[1]}`) {
  main().catch(err => {
    console.error('[Digest Daemon] 啟動失敗:', err);
    process.exit(1);
  });
}

export default createDaemon;
