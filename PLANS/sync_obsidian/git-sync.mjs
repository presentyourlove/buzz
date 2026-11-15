import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';

/**
 * 執行 Git 指令的輔助函式
 * @param {string} cwd - 工作目錄
 * @param {string[]} args - 指令參數
 * @returns {{ status: number, stdout: string, stderr: string }}
 */
function runGit(cwd, args) {
  const result = spawnSync('git', args, {
    cwd,
    encoding: 'utf8',
    timeout: 10000,
    windowsHide: true,
  });
  return {
    status: result.status ?? 1,
    stdout: (result.stdout || '').trim(),
    stderr: (result.stderr || '').trim(),
  };
}

/**
 * 本地 Git 版本控制同步器
 */
export class GitSync {
  constructor(config = {}) {
    this.vaultPath = config.vault_path || 'c:/私人/ai agent/buzz-obsidian';
    this.gitConfig = config.git || {
      auto_commit: true,
      auto_push: false,
      remote: '',
      branch: 'main',
      init_if_missing: true,
    };
  }

  /**
   * 確保 Vault 具備 Git 倉儲初始化
   */
  ensureGitRepo() {
    const gitDir = path.resolve(this.vaultPath, '.git');
    if (!fs.existsSync(gitDir)) {
      if (this.gitConfig.init_if_missing) {
        console.log(`[git-sync] 初始化本地 Git 倉儲: ${this.vaultPath}`);
        const branch = this.gitConfig.branch || 'main';
        runGit(this.vaultPath, ['init', '-b', branch]);

        // 設定本機身分識別，防止無全域 git config 時 commit 失敗
        runGit(this.vaultPath, ['config', 'user.name', 'Buzz Sync Agent']);
        runGit(this.vaultPath, ['config', 'user.email', 'buzz-sync@local.internal']);
      } else {
        throw new Error(`Vault 未包含 Git 倉儲且未開啟 init_if_missing: ${this.vaultPath}`);
      }
    }
  }

  /**
   * 檢查是否有檔案變動
   * @returns {boolean}
   */
  hasChanges() {
    const res = runGit(this.vaultPath, ['status', '--porcelain']);
    return res.status === 0 && res.stdout.length > 0;
  }

  /**
   * 執行自動提交
   * @param {string} [customMessage] - 自訂 commit message
   * @returns {{ success: boolean, committed: boolean, commitHash?: string, message?: string }}
   */
  commitChanges(customMessage) {
    if (!this.gitConfig.auto_commit) {
      return { success: true, committed: false, message: 'auto_commit 已關閉' };
    }

    this.ensureGitRepo();

    if (!this.hasChanges()) {
      return { success: true, committed: false, message: '無工作目錄變動，跳過提交' };
    }

    // 1. Stage 所有變動
    const addRes = runGit(this.vaultPath, ['add', '-A']);
    if (addRes.status !== 0) {
      return { success: false, committed: false, message: `git add 失敗: ${addRes.stderr}` };
    }

    // 2. 建立提交訊息
    const nowIso = new Date().toISOString();
    const commitMsg = customMessage || `chore(sync): ${nowIso} auto-sync Core Memory from Buzz`;

    const commitRes = runGit(this.vaultPath, ['commit', '-m', commitMsg]);
    if (commitRes.status !== 0) {
      return { success: false, committed: false, message: `git commit 失敗: ${commitRes.stderr}` };
    }

    // 取得最新 commit hash
    const revRes = runGit(this.vaultPath, ['rev-parse', '--short', 'HEAD']);
    const commitHash = revRes.status === 0 ? revRes.stdout : 'unknown';

    console.log(`[git-sync] 成功完成本地 Git 提交: [${commitHash}] ${commitMsg}`);

    // 3. 若開啟 auto_push，推送至 GitHub 遠端倉庫
    let pushed = false;
    if (this.gitConfig.auto_push) {
      const remote = this.gitConfig.remote || 'origin';
      const branch = this.gitConfig.branch || 'main';
      console.log(`[git-sync] 正在自動推送變動至 GitHub (${remote}/${branch})...`);
      const pushRes = runGit(this.vaultPath, ['push', remote, branch]);
      if (pushRes.status === 0) {
        pushed = true;
        console.log(`[git-sync] ✔ 成功推送至 GitHub 遠端私有倉庫！`);
      } else {
        console.warn(`[git-sync] ⚠ 推送至 GitHub 失敗: ${pushRes.stderr || pushRes.stdout}`);
      }
    }

    return {
      success: true,
      committed: true,
      commitHash,
      pushed,
      message: commitMsg,
    };
  }
}
