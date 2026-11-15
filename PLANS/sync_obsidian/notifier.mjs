import { spawn } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';

/**
 * Wax 完工主動廣播模組 (Wax Notifier)
 * 負責在記憶提煉、歸檔並推送至 GitHub 後，主動向老闆發布多管道完工通報
 */
export class WaxNotifier {
  constructor(config = {}) {
    this.config = config;
    this.obsidianPath = config.obsidian?.vault_path || 'C:/私人/ai agent/buzz-obsidian';
    this.broadcastLogPath = path.join(this.obsidianPath, 'wiki', '02-Conversation-Digest', 'WAX_BROADCAST_LOG.md');
  }

  /**
   * 發送 Windows 原生桌面 Toast 橫幅推播通知
   * @param {string} title - 通知標題
   * @param {string} message - 通知內容
   */
  sendWindowsToast(title, message) {
    try {
      // 使用 PowerShell 原生系統通知（安全、免第三方依賴、Windows 10/11 原生相容）
      const psScript = `
[Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] > $null
$template = [Windows.UI.Notifications.ToastNotificationManager]::GetTemplateContent([Windows.UI.Notifications.ToastTemplateType]::ToastText02)
$textNodes = $template.GetElementsByTagName('text')
$textNodes.Item(0).AppendChild($template.CreateTextNode('${title.replace(/'/g, "''")}')) > $null
$textNodes.Item(1).AppendChild($template.CreateTextNode('${message.replace(/'/g, "''")}')) > $null
$notifier = [Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier('Buzz 第二大腦 (Wax)')
$notification = [Windows.UI.Notifications.ToastNotification]::new($template)
$notifier.Show($notification)
      `.trim();

      const child = spawn('powershell', ['-NoProfile', '-WindowStyle', 'Hidden', '-Command', psScript], {
        detached: true,
        stdio: 'ignore',
        windowsHide: true,
      });
      child.unref();
    } catch (err) {
      console.warn(`[WaxNotifier] Windows 桌面通知發送略過: ${err.message}`);
    }
  }

  /**
   * 寫入 Wax 廣播日誌至 Obsidian 第二大腦
   * @param {object} report - 廣播內容物件
   */
  appendObsidianBroadcastLog(report) {
    try {
      const nowStr = new Date().toLocaleString('zh-TW', { timeZone: 'Asia/Taipei' });
      const dir = path.dirname(this.broadcastLogPath);
      if (!fs.existsSync(dir)) {
        fs.mkdirSync(dir, { recursive: true });
      }

      let content = '';
      if (!fs.existsSync(this.broadcastLogPath)) {
        content = `# 🧹 Wax 記憶清潔官 — 完工廣播與巡檢歷史日誌\n\n> 本日誌由 Wax 記憶整理引擎每 30 分鐘背景巡檢時自動產生，記錄所有歷史記憶提煉、冷記憶索引與 GitHub 推送紀錄。\n\n---\n\n`;
      }

      content += `### 【${nowStr}】Wax 記憶巡檢完工快報 🚀\n\n`;
      content += `- **核心記憶狀態**：${report.coreChanged ? '✨ 已更新並過濾機敏資料' : '維持最新'}\n`;
      content += `- **冷記憶規格書**：共掃描 ${report.coldCount} 份規格文檔 (索引已同步)\n`;
      content += `- **對話脈絡提煉**：共結構化歸檔 ${report.digestCount} 篇討論串\n`;
      content += `- **GitHub 備份**：${report.committed ? '✅ 已自動建立 Git Commit 並成功推送到私有倉庫 (`presentyourlove/buzz-obsidian`)' : '筆記無實質變更，維持現狀'}\n\n---\n\n`;

      fs.appendFileSync(this.broadcastLogPath, content, 'utf8');
      console.log(`✔ Wax 廣播日誌已寫入: wiki/02-Conversation-Digest/WAX_BROADCAST_LOG.md`);
    } catch (err) {
      console.warn(`[WaxNotifier] 寫入廣播日誌失敗: ${err.message}`);
    }
  }

  /**
   * 執行全管道主動廣播
   * @param {object} stats - 同步統計資訊
   */
  broadcast(stats = {}) {
    const title = '🧹 Wax 記憶清潔官快報';
    const message = stats.committed
      ? `主理人好！剛才已完成第二大腦記憶整理，並已自動推送至 GitHub 私有倉庫！`
      : `主理人好！過去 30 分鐘對話已巡檢完畢，第二大腦記憶庫維持最新狀態！`;

    console.log(`\n📢 正在發布 Wax 完工廣播...`);
    this.sendWindowsToast(title, message);
    this.appendObsidianBroadcastLog(stats);
    console.log(`✔ Wax 完工廣播已成功推送到桌面與第二大腦！\n`);
  }
}
