import fs from 'node:fs';
import path from 'node:path';

/**
 * 同步狀態持久化與心跳記錄模組
 */
export class StateStore {
  constructor(config = {}) {
    this.vaultPath = config.vault_path || 'c:/私人/ai agent/buzz-obsidian';
    const metaDirRel = config.vault_structure?.sync_meta_dir || 'wiki/00-Core-Memory/sync-meta';
    this.metaDir = path.resolve(this.vaultPath, metaDirRel);

    this.ensureDir();
  }

  ensureDir() {
    if (!fs.existsSync(this.metaDir)) {
      fs.mkdirSync(this.metaDir, { recursive: true });
    }
  }

  /**
   * 寫入最後同步游標資訊
   * @param {object} syncRecord - 同步記錄資料
   */
  updateLastSync(syncRecord) {
    this.ensureDir();
    const filePath = path.join(this.metaDir, 'last-sync.json');
    const data = {
      updated_at: new Date().toISOString(),
      ...syncRecord,
    };
    fs.writeFileSync(filePath, JSON.stringify(data, null, 2), 'utf8');
  }

  /**
   * 更新服務心跳檔
   */
  writeHeartbeat() {
    this.ensureDir();
    const filePath = path.join(this.metaDir, 'heartbeat.json');
    const data = {
      timestamp: new Date().toISOString(),
      pid: process.pid,
      uptime_seconds: Math.floor(process.uptime()),
      status: 'alive',
    };
    fs.writeFileSync(filePath, JSON.stringify(data, null, 2), 'utf8');
  }

  /**
   * 記錄衝突或異常訊息
   * @param {object} logEntry - 異常日誌項目
   */
  appendConflictLog(logEntry) {
    this.ensureDir();
    const filePath = path.join(this.metaDir, 'conflict-log.json');
    let logs = [];
    if (fs.existsSync(filePath)) {
      try {
        logs = JSON.parse(fs.readFileSync(filePath, 'utf8'));
      } catch {
        logs = [];
      }
    }
    logs.push({
      timestamp: new Date().toISOString(),
      ...logEntry,
    });
    fs.writeFileSync(filePath, JSON.stringify(logs.slice(-100), null, 2), 'utf8');
  }
}
