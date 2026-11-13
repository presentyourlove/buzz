/**
 * Obsidian Digest Sync Skill
 * 核心功能：自動化對話摘要生成與 Obsidian Vault 雙向知識庫同步
 * 
 * 用法：
 *   node main.js --run-once --date 2026-11-01    # 單次收割指定日期
 *   node main.js --run-once                       # 單次收割昨天
 *   node main.js --batch --start 2026-11-01 --end 2026-11-03  # 批次收割
 *   node main.js --daemon                         # 常駐模式 (Phase B)
 */

import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { DigestSync } from './digest_sync.mjs';
import { ObsidianDigestSync as LegacyObsidianDigestSync } from './main_legacy.js';
import { createDaemon } from './digest_daemon.mjs';

export { DigestSync } from './digest_sync.mjs';
export { ObsidianDigestSync as LegacyObsidianDigestSync } from './main_legacy.js';

// 向後相容：保留原有類別名稱
export class ObsidianDigestSync extends LegacyObsidianDigestSync {
  constructor(config = {}) {
    super(config);
    this.digestSync = new DigestSync(config);
  }

  async syncToVault(dateStr, threadData) {
    // 使用新的協調器進行完整同步
    const result = await this.digestSync.runOnce(dateStr);
    return result;
  }
}

// CLI 入口
async function main() {
  const args = process.argv.slice(2);
  const config = {
    channel_id: process.env.BUZZ_DIGEST_SYNC_CHANNEL || '5ca019f6-1a27-592a-b015-4baf0eb33572',
    vault_path: process.env.BUZZ_OBSIDIAN_VAULT_PATH || 'C:/私人/ai agent/buzz-obsidian',
    output_dir: process.env.BUZZ_DIGEST_OUTPUT_DIR || 'wiki/02-Conversation-Digest',
    sync_command: process.env.BUZZ_SYNC_OBSIDIAN_CMD || 'node PLANS/sync_obsidian/main.mjs',
    working_dir: process.env.BUZZ_SYNC_WORKING_DIR || 'C:/Users/user/.buzz'
  };

  const digestSync = new DigestSync(config);

  // 解析參數
  const runOnce = args.includes('--run-once');
  const batch = args.includes('--batch');
  const daemon = args.includes('--daemon');
  const dateIdx = args.indexOf('--date');
  const startIdx = args.indexOf('--start');
  const endIdx = args.indexOf('--end');

  const dateStr = dateIdx !== -1 ? args[dateIdx + 1] : null;
  const startDate = startIdx !== -1 ? args[startIdx + 1] : null;
  const endDate = endIdx !== -1 ? args[endIdx + 1] : null;

  if (runOnce) {
    const result = await digestSync.runOnce(dateStr);
    process.exit(result.success ? 0 : 1);
  } else if (batch && startDate && endDate) {
    const results = await digestSync.runBatch(startDate, endDate);
    const failed = results.filter(r => !r.success).length;
    console.log(`\n批次完成：共 ${results.length} 天，失敗 ${failed} 天`);
    process.exit(failed > 0 ? 1 : 0);
  } else if (daemon) {
    const daemonInstance = createDaemon(config);
    
    // 優雅關閉處理
    const shutdown = () => {
      console.log(`\n[Obsidian Digest Sync] 接收到關閉信號，正在優雅關閉...`);
      daemonInstance.stop();
      process.exit(0);
    };
    
    process.on('SIGINT', shutdown);
    process.on('SIGTERM', shutdown);
    
    // 啟動 Daemon
    daemonInstance.start();
    console.log(`[Obsidian Digest Sync] 常駐模式已啟動，Cron 排程: ${daemonInstance.cronSchedule}`);
    console.log(`[Obsidian Digest Sync] 按 Ctrl+C 停止`);
    
    // 保持進程存活
    await new Promise(() => {});
  } else {
    console.log(`
Obsidian Digest Sync Skill

用法：
  node main.js --run-once [--date YYYY-MM-DD]     單次收割 (預設昨天)
  node main.js --batch --start YYYY-MM-DD --end YYYY-MM-DD  批次收割
  node main.js --daemon                           常駐模式 (Phase B)

環境變數：
  BUZZ_DIGEST_SYNC_CHANNEL    頻道 ID (預設: 5ca019f6-1a27-592a-b015-4baf0eb33572)
  BUZZ_OBSIDIAN_VAULT_PATH    Vault 路徑 (預設: c:/私人/ai agent/buzz-obsidian)
  BUZZ_DIGEST_OUTPUT_DIR      輸出目錄 (預設: wiki/02-Conversation-Digest)
  BUZZ_SYNC_OBSIDIAN_CMD      同步命令 (預設: node PLANS/sync_obsidian/main.mjs)
  BUZZ_SYNC_WORKING_DIR       工作目錄 (預設: /c/Users/user/.buzz)
`);
    process.exit(0);
  }
}

// 只有直接執行時才啟動 CLI
function isDirectRun() {
  if (!process.argv[1]) return false;
  try {
    const scriptPath = path.resolve(process.argv[1]).toLowerCase();
    const currentPath = fileURLToPath(import.meta.url).toLowerCase();
    return scriptPath === currentPath;
  } catch (e) {
    return false;
  }
}

if (isDirectRun()) {
  main().catch(err => {
    console.error('錯誤:', err);
    process.exit(1);
  });
}

export default DigestSync;
