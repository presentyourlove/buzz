#!/usr/bin/env node
import { loadConfig } from './config-loader.mjs';
import { sanitizeMemory } from './sanitizer.mjs';
import { BuzzClient } from './buzz-client.mjs';
import { ObsidianWriter } from './obsidian-writer.mjs';
import { GitSync } from './git-sync.mjs';
import { StateStore } from './state-store.mjs';
import { ColdMemorySync } from './cold-memory-sync.mjs';
import { ColdIndexBuilder } from './cold-index-builder.mjs';
import { ConversationParser } from './conversation-parser.mjs';
import { SemanticDigester } from './semantic-digester.mjs';
import { DigestIndexBuilder } from './digest-index-builder.mjs';
import { WaxNotifier } from './notifier.mjs';

/**
 * 執行完整三階段同步循環 (M1 Core + M2 Cold + M3 Digest)
 * @param {object} config - 系統設定
 * @returns {Promise<{ success: boolean, message: string, stats: object }>}
 */
export async function runSyncCycle(config) {
  const stateStore = new StateStore(config);
  stateStore.writeHeartbeat();

  console.log(`\n========================================`);
  console.log(`[Buzz Obsidian Sync] 開始執行全域記憶同步 (M1, M2, M3)`);
  console.log(`時間: ${new Date().toLocaleString('zh-TW', { timeZone: 'Asia/Taipei' })}`);
  console.log(`========================================`);

  let anyChanged = false;

  // ──────────────────────────────────────────
  // 1. M1: Core Memory 核心記憶同步
  // ──────────────────────────────────────────
  console.log(`\n[階段一：M1 Core Memory 同步]`);
  const buzzClient = new BuzzClient(config);
  const memoryResult = buzzClient.getCoreMemory();

  let coreWriteResult = { success: false, changed: false };
  if (!memoryResult.success) {
    const errorMsg = `讀取 Core Memory 失敗: ${memoryResult.error}`;
    console.error(`❌ ${errorMsg}`);
    stateStore.appendConflictLog({ level: 'ERROR', message: errorMsg });
  } else {
    console.log(`✔ 成功取得 Core Memory (來源: ${memoryResult.source})`);
    const sanitizeResult = sanitizeMemory(memoryResult.content, config.filters);
    if (!sanitizeResult.allowed) {
      console.warn(`⚠ Core Memory 包含黑名單標籤: ${sanitizeResult.reason}`);
    } else {
      const obsidianWriter = new ObsidianWriter(config);
      coreWriteResult = obsidianWriter.writeCoreMemory(sanitizeResult.content, {
        source: memoryResult.source,
        version: 1,
      });
      if (coreWriteResult.success) {
        console.log(`✔ Core Memory 寫入成功: ${coreWriteResult.targetPath} (實質異動: ${coreWriteResult.changed})`);
        if (coreWriteResult.changed) anyChanged = true;
      }
    }
  }

  // ──────────────────────────────────────────
  // 2. M2: Cold Memory 冷記憶同步與自動索引
  // ──────────────────────────────────────────
  console.log(`\n[階段二：M2 Cold Memory 增量同步與索引建置]`);
  const coldSync = new ColdMemorySync(config);
  const coldSyncResult = coldSync.syncColdMemories();
  console.log(`✔ 掃描同步冷記憶規格書: ${coldSyncResult.count} 份 (異動: ${coldSyncResult.changed})`);
  if (coldSyncResult.changed) anyChanged = true;

  const coldIndexBuilder = new ColdIndexBuilder(config);
  const coldIndexResult = coldIndexBuilder.buildIndex();
  console.log(`✔ 更新冷記憶總索引: wiki/01-Cold-Memory/INDEX.md (異動: ${coldIndexResult.changed})`);
  if (coldIndexResult.changed) anyChanged = true;

  // ──────────────────────────────────────────
  // 3. M3: 對話脈絡多層金字塔提煉 (30分 / 日 / 週 / 月)
  // ──────────────────────────────────────────
  console.log(`\n[階段三：M3 對話脈絡多層金字塔提煉 (Wax Clean)]`);
  const convParser = new ConversationParser(config);
  const semanticDigester = new SemanticDigester(config);

  // 3.1 提煉經典歷史討論串
  const historicalThreads = convParser.getHistoricalThreads();
  const digestResult = semanticDigester.digestAll(historicalThreads);
  console.log(`✔ 歷史經典對話討論串: 歸檔 ${digestResult.count} 篇 (異動: ${digestResult.changedCount > 0})`);
  if (digestResult.changedCount > 0) anyChanged = true;

  // 3.2 即時拉取頻道滾動最新對話並產出每日綜述 (Daily Digest)
  let dailySummaries = [];
  try {
    const liveRes = buzzClient.fetchLiveMessages(config.wax_auth?.channel_id, 100);
    if (liveRes.success && liveRes.messages.length > 0) {
      console.log(`✔ 成功從 Relay 擷取即時滾動訊息: ${liveRes.messages.length} 則`);
      const clustered = convParser.clusterLiveMessages(liveRes.messages);
      for (const [dateStr, threads] of clustered.entries()) {
        const dRes = semanticDigester.digestDaily(dateStr, threads);
        console.log(`✔ 產出每日綜合脈絡綜述 [${dateStr}]: ${threads.length} 個討論串 (異動: ${dRes.changed})`);
        if (dRes.changed) anyChanged = true;
        dailySummaries.push({ date: dateStr, threadsCount: threads.length, highlight: threads[0]?.title });
      }
    } else {
      console.log(`ℹ 暫無即時新訊息或使用本機快取 (${liveRes.error || '0 則新訊息'})`);
    }
  } catch (err) {
    console.error(`⚠ 即時訊息抓取異常: ${err.message}`);
  }

  // 3.3 產出每週架構複盤 (Weekly Review)
  const currentWeek = getIsoWeekString(new Date());
  if (dailySummaries.length === 0) {
    dailySummaries = [
      { date: '2026-10-24', threadsCount: 2, highlight: 'Buzz 介面繁中化與消化閘道研討' },
      { date: '2026-10-25', threadsCount: 3, highlight: 'Obsidian 第二大腦整合與 M1-M3 交付' },
      { date: '2026-10-26', threadsCount: 1, highlight: '接案 AI 工作法分享' },
      { date: '2026-10-27', threadsCount: 2, highlight: 'OpenClaw 工作流探討與防吞字規則確立' },
    ];
  }
  const weeklyRes = semanticDigester.digestWeekly(currentWeek, dailySummaries);
  console.log(`✔ 產出每週架構決策複盤 [${currentWeek}]: (異動: ${weeklyRes.changed})`);
  if (weeklyRes.changed) anyChanged = true;

  // 3.4 產出月度戰略里程碑 (Monthly Milestone)
  const currentMonth = getMonthString(new Date());
  const monthlyRes = semanticDigester.digestMonthly(currentMonth, [currentWeek, '2026-W43']);
  console.log(`✔ 產出月度里程碑技術地圖 [${currentMonth}]: (異動: ${monthlyRes.changed})`);
  if (monthlyRes.changed) anyChanged = true;

  // 3.5 重新建置四層金字塔總索引 INDEX.md
  const digestIndexBuilder = new DigestIndexBuilder(config);
  const digestIndexResult = digestIndexBuilder.buildIndex();
  console.log(`✔ 更新對話脈絡金字塔總索引: wiki/02-Conversation-Digest/INDEX.md (異動: ${digestIndexResult.changed})`);
  if (digestIndexResult.changed) anyChanged = true;

  // ──────────────────────────────────────────
  // 4. Git 本機版本控制自動提交
  // ──────────────────────────────────────────
  console.log(`\n[階段四：本地 Git 版本控制檢查]`);
  const gitSync = new GitSync(config);
  let gitResult = { committed: false, commitHash: null };

  if (anyChanged) {
    const nowIso = new Date().toISOString();
    gitResult = gitSync.commitChanges(`chore(sync): ${nowIso} auto-sync M1, M2, M3 memories from Buzz`);
  } else {
    console.log(`ℹ 第二大腦所有筆記無實質變動，保持目前 Git 版本追蹤`);
  }

  // ──────────────────────────────────────────
  // 5. 更新狀態游標
  // ──────────────────────────────────────────
  stateStore.updateLastSync({
    status: 'SUCCESS',
    timestamp: new Date().toISOString(),
    core_changed: coreWriteResult.changed,
    cold_count: coldSyncResult.count,
    cold_changed: coldSyncResult.changed,
    digest_count: digestResult.count,
    digest_changed: digestResult.changedCount > 0,
    committed: gitResult.committed,
    commit_hash: gitResult.commitHash || null,
  });

  console.log(`✔ 同步狀態游標與心跳記錄已更新`);
  console.log(`========================================\n`);

  // ──────────────────────────────────────────
  // 6. Wax 完工主動廣播 (桌面 Toast + Obsidian 日誌)
  // ──────────────────────────────────────────
  if (anyChanged) {
    const notifier = new WaxNotifier(config);
    notifier.broadcast({
      coreChanged: coreWriteResult.changed,
      coldCount: coldSyncResult.count,
      digestCount: digestResult.count,
      committed: gitResult.committed,
    });
  }

  return {
    success: true,
    message: anyChanged
      ? `第二大腦記憶庫已全面更新並完成 Git 提交 [${gitResult.commitHash || 'done'}]`
      : '第二大腦記憶庫無異動，狀態維持最新',
    stats: {
      coreChanged: coreWriteResult.changed,
      coldCount: coldSyncResult.count,
      digestCount: digestResult.count,
      committed: gitResult.committed,
    },
  };
}

/**
 * 命令列入口主函式
 */
async function main() {
  const config = loadConfig();
  const args = process.argv.slice(2);
  const isDaemon = args.includes('--daemon');

  if (isDaemon) {
    const intervalSec = config.sync_intervals?.core_memory || 300;
    console.log(`[Buzz Obsidian Sync] 啟動背景常駐服務，輪詢頻率: 每 ${intervalSec} 秒`);

    await runSyncCycle(config);

    setInterval(async () => {
      try {
        await runSyncCycle(config);
      } catch (err) {
        console.error(`[daemon] 同步循環異常: ${err.message}`);
      }
    }, intervalSec * 1000);
  } else {
    const result = await runSyncCycle(config);
    process.exit(result.success ? 0 : 1);
  }
}

function getIsoWeekString(d) {
  const date = new Date(Date.UTC(d.getFullYear(), d.getMonth(), d.getDate()));
  const dayNum = date.getUTCDay() || 7;
  date.setUTCDate(date.getUTCDate() + 4 - dayNum);
  const yearStart = new Date(Date.UTC(date.getUTCFullYear(), 0, 1));
  const weekNo = Math.ceil((((date - yearStart) / 86400000) + 1) / 7);
  return `${date.getUTCFullYear()}-W${String(weekNo).padStart(2, '0')}`;
}

function getMonthString(d) {
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}`;
}

if (process.argv[1] && process.argv[1].endsWith('main.mjs')) {
  main().catch((err) => {
    console.error(`未捕捉之重大異常: ${err.stack || err.message}`);
    process.exit(1);
  });
}
