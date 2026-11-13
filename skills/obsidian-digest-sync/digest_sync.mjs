/**
 * Digest Sync - 主協調器
 * 串聯 Collector -> Extractor -> Formatter -> Writer -> Syncer -> Notifier
 */

import { DigestCollector } from './digest_collector.mjs';
import { DigestExtractor } from './digest_extractor.mjs';
import { DigestFormatter } from './digest_formatter.mjs';
import { ObsidianWriter } from './obsidian_writer.mjs';
import { execFileSync } from 'node:child_process';
import path from 'node:path';

export class DigestSync {
  constructor(config = {}) {
    this.config = {
      channel_id: config.channel_id || '5ca019f6-1a27-592a-b015-4baf0eb33572',
      vault_path: config.vault_path || 'c:/私人/ai agent/buzz-obsidian',
      output_dir: config.output_dir || 'wiki/02-Conversation-Digest',
      sync_command: config.sync_command || 'node PLANS/sync_obsidian/main.mjs',
      working_dir: config.working_dir || (process.platform === 'win32' ? 'C:/Users/user/.buzz' : '/c/Users/user/.buzz'),
      enabled: config.enabled !== false
    };

    // 初始化各組件
    this.collector = new DigestCollector({
      channel_id: this.config.channel_id,
      working_dir: this.config.working_dir
    });
    this.extractor = new DigestExtractor({});
    this.formatter = new DigestFormatter({
      output_dir: this.config.output_dir
    });
    this.writer = new ObsidianWriter({
      vault_path: this.config.vault_path,
      output_dir: this.config.output_dir
    });
  }

  /**
   * 執行單次收割 (CLI --run-once 模式)
   * @param {string} dateStr - YYYY-MM-DD，預設為昨天
   * @returns {Promise<SyncResult>} 同步結果
   */
  async runOnce(dateStr = null) {
    const targetDate = dateStr || this.getYesterday();
    console.log(`\n========================================`);
    console.log(`[Obsidian Digest Sync] 開始單次收割: ${targetDate}`);
    console.log(`========================================\n`);

    try {
      // 1. 收集訊息
      console.log(`[1/6] 正在收集 ${targetDate} 的頻道訊息...`);
      const channelMessages = await this.collector.collect(targetDate);
      console.log(`    ✔ 取得 ${channelMessages.message_count} 則訊息，${channelMessages.participant_count} 位參與者`);

      if (channelMessages.message_count === 0) {
        return {
          success: true,
          message: `無訊息可萃取 (${targetDate})`,
          date: targetDate,
          message_count: 0
        };
      }

      // 2. LLM 萃取
      console.log(`[2/6] 正在進行 LLM 萃取...`);
      const extraction = await this.extractor.extract(channelMessages);
      console.log(`    ✔ 萃取完成：${extraction.key_decisions.length} 決策、${extraction.architecture_consensus.length} 共識、${extraction.action_items.length} 待辦`);

      // 3. 格式化
      console.log(`[3/6] 正在格式化 Markdown...`);
      const digestOutput = this.formatter.format(extraction, channelMessages);
      console.log(`    ✔ 產出檔案：${digestOutput.fileName}`);

      // 4. 寫入 Vault
      console.log(`[4/6] 正在寫入 Obsidian Vault...`);
      const writeResult = this.writer.writeDigest(digestOutput);
      if (!writeResult.success) {
        throw new Error(writeResult.error);
      }
      console.log(`    ✔ 寫入 ${writeResult.changed ? '新增/更新' : '無變動'}：${writeResult.fileName}`);

      // 5. 更新 INDEX.md
      console.log(`[5/6] 正在更新 INDEX.md...`);
      const indexResult = this.writer.updateIndex([digestOutput]);
      if (!indexResult.success) {
        throw new Error(indexResult.error);
      }
      console.log(`    ✔ 索引 ${indexResult.changed ? '已更新' : '無變動'}，共 ${indexResult.count} 篇`);

      // 6. 執行同步 (呼叫現有的 sync_obsidian/main.mjs)
      console.log(`[6/6] 正在執行雙向同步...`);
      const syncResult = await this.runSync();
      console.log(`    ✔ 同步完成：${syncResult.message}`);

      // 完成通知
      const result = {
        success: true,
        date: targetDate,
        fileName: digestOutput.fileName,
        filePath: writeResult.filePath,
        extraction,
        writeResult,
        indexResult,
        syncResult,
        message: `完成收割 ${targetDate}，產出 ${digestOutput.fileName}`
      };

      console.log(`\n========================================`);
      console.log(`✅ 收割完成：${result.message}`);
      console.log(`========================================\n`);

      return result;

    } catch (error) {
      console.error(`\n❌ 收割失敗: ${error.message}`);
      console.log(`========================================\n`);
      return {
        success: false,
        date: targetDate,
        error: error.message
      };
    }
  }

  /**
   * 執行雙向同步 (呼叫現有的 sync_obsidian/main.mjs，使用 execFileSync 防止命令注入)
   */
  async runSync() {
    try {
      const cmdParts = String(this.config.sync_command || 'node PLANS/sync_obsidian/main.mjs').trim().split(/\s+/);
      const bin = cmdParts[0];
      const args = cmdParts.slice(1);
      const output = execFileSync(bin, args, {
        cwd: this.config.working_dir,
        encoding: 'utf8',
        maxBuffer: 10 * 1024 * 1024,
        timeout: 120000, // 2 分鐘超時
        env: { ...process.env }
      });
      return { success: true, message: '同步成功', output };
    } catch (error) {
      // 同步失敗不阻斷主流程，只記錄
      return { success: false, message: `同步失敗: ${error.message}`, error };
    }
  }

  /**
   * 發布完成通知到頻道 (Kind:1 事件)
   * 這裡只是準備通知內容，實際發送需整合 buzz-agent 的 builtin 發送機制
   */
  prepareNotification(result) {
    if (!result.success) return null;

    const { date, fileName, extraction } = result;
    const decisions = extraction.key_decisions.slice(0, 3).join('；');
    
    return {
      content: `✅ **每日對話摘要已沉澱**\n\n` +
        `📅 **日期**：${date}\n` +
        `📝 **檔案**：\`wiki/02-Conversation-Digest/${fileName}\`\n` +
        `🎯 **核心決策**：${decisions || '無重大決策'}\n` +
        `🔗 **索引**：[[wiki/02-Conversation-Digest/INDEX|對話脈絡總索引]]\n\n` +
        `#buzz-digest #auto-sync`,
      tags: [
        ['h', this.config.channel_id],
        ['t', 'buzz-digest'],
        ['t', 'auto-sync']
      ]
    };
  }

  /**
   * 取得昨天日期字串
   */
  getYesterday() {
    const yesterday = new Date();
    yesterday.setDate(yesterday.getDate() - 1);
    return yesterday.toISOString().slice(0, 10);
  }

  /**
   * 批次處理多天 (補齊歷史)
   */
  async runBatch(startDate, endDate) {
    const results = [];
    const start = new Date(startDate);
    const end = new Date(endDate);
    
    for (let d = new Date(start); d <= end; d.setDate(d.getDate() + 1)) {
      const dateStr = d.toISOString().slice(0, 10);
      const result = await this.runOnce(dateStr);
      results.push(result);
      // 避免過快請求
      await new Promise(r => setTimeout(r, 1000));
    }
    
    return results;
  }
}

export default DigestSync;
