import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const REPO_ROOT = path.resolve(__dirname, '../..');
const WORK_LOGS_DIR = path.join(REPO_ROOT, 'WORK_LOGS');
const FAILURE_LOG = path.join(WORK_LOGS_DIR, 'daily_evolution_failure.log');

// Target Channel ID requested by Boss
const TARGET_CHANNEL_ID = process.env.BUZZ_CHANNEL_ID || '5ca019f6-1a27-592a-b015-4baf0eb33572';

const ENV = {
  ...process.env,
  BUZZ_CHANNEL_ID: TARGET_CHANNEL_ID,
  BUZZ_RELAY_URL: process.env.BUZZ_RELAY_URL || 'wss://presentyourlove.communities.buzz.xyz',
  BUZZ_PRIVATE_KEY: process.env.BUZZ_PRIVATE_KEY || 'nsec1ly388ent2cvr7f9rscxffxgmvuljggxx374sjkfvwee4zrl3p89sqfjmce',
  BUZZ_AUTH_TAG: process.env.BUZZ_AUTH_TAG || '["auth","2d29f28fd5106795c630120d7e5b062bc4f5caa8057b4a6a374b7468cdc5de8e","","6f67f68423fe1877d312046cc63cf0106b7f31f284eb51d586dc036a235aeb8433d159d97c37e2b927b40fdc68eff0e1071fdaf56c497494d1779a8bc7fc6118"]',
  BUZZ_CLI: process.env.BUZZ_CLI || 'C:/Users/user/AppData/Local/Buzz/buzz.exe',
  BUZZ_CLI_PATH: process.env.BUZZ_CLI_PATH || 'C:/Users/user/AppData/Local/Buzz/buzz.exe',
  PYTHONIOENCODING: 'utf-8',
  PYTHONUTF8: '1',
};

// Validate required environment variables
const requiredEnvVars = ['BUZZ_PRIVATE_KEY', 'BUZZ_AUTH_TAG'];
for (const varName of requiredEnvVars) {
  if (!ENV[varName]) {
    error(`Missing required environment variable: ${varName}. Must be provided by deployment environment.`);
    process.exit(2);
  }
}

function log(msg) {
  const ts = new Date().toLocaleString('zh-TW', { timeZone: 'Asia/Taipei' });
  console.log(`[DailyEvolution] [${ts}] ${msg}`);
}

function error(msg) {
  const ts = new Date().toLocaleString('zh-TW', { timeZone: 'Asia/Taipei' });
  console.error(`[DailyEvolution] ❌ [${ts}] ERROR: ${msg}`);
}

async function main() {
  log(`Starting Daily Evolution Pipeline for Channel: ${TARGET_CHANNEL_ID}`);
  log(`Workspace Root: ${REPO_ROOT}`);

  if (!fs.existsSync(WORK_LOGS_DIR)) {
    fs.mkdirSync(WORK_LOGS_DIR, { recursive: true });
  }

  // Circuit breaker check
  if (fs.existsSync(FAILURE_LOG)) {
    const lines = fs.readFileSync(FAILURE_LOG, 'utf8').trim().split('\n').filter(Boolean);
    if (lines.length >= 2) {
      error(`Circuit breaker tripped! (${lines.length} consecutive failures). Stopping.`);
      console.log(`Please inspect ${FAILURE_LOG} and clear it after fixing issues.`);
      process.exit(3);
    }
  }

  // 1. Calculate today's start timestamp (Asia/Taipei)
  const now = new Date();
  const taipeiDateStr = now.toLocaleDateString('sv-SE', { timeZone: 'Asia/Taipei' }); // YYYY-MM-DD
  const todayStartUtc = Math.floor(new Date(`${taipeiDateStr}T00:00:00+08:00`).getTime() / 1000);
  log(`Harvesting messages since: ${todayStartUtc} (${taipeiDateStr} 00:00:00+08:00)`);

  // 2. Run Daily Harvest (TypeScript via Node)
  log('Step 1/2: Running daily harvest script...');
  const harvestScript = path.join(REPO_ROOT, 'scripts/daily-harvest.ts');
  const harvestArgs = [
    '--experimental-strip-types',
    harvestScript,
    '--channel', TARGET_CHANNEL_ID,
    '--since', String(todayStartUtc),
    '--out-dir', WORK_LOGS_DIR
  ];

  const harvestRes = spawnSync(process.execPath, harvestArgs, {
    env: ENV,
    cwd: REPO_ROOT,
    encoding: 'utf8'
  });

  if (harvestRes.status !== 0) {
    error(`Harvest failed: ${harvestRes.stderr || harvestRes.stdout}`);
    fs.appendFileSync(FAILURE_LOG, `${new Date().toISOString()} HARVEST_FAILED ${harvestRes.status}\n`);
    process.exit(harvestRes.status || 1);
  }

  log('Harvest finished successfully.');
  console.log(harvestRes.stdout.trim());

  // 3. Run Daily Evolution Proposal Generation (Python)
  log('Step 2/2: Generating daily evolution proposal & posting to channel...');
  const pythonScript = path.join(REPO_ROOT, 'scripts/daily_evolution/daily_evolution.py');
  const pythonArgs = [
    pythonScript,
    '--channel', TARGET_CHANNEL_ID,
    '--relay-url', ENV.BUZZ_RELAY_URL,
    '--private-key', ENV.BUZZ_PRIVATE_KEY,
    '--buzz-cli', ENV.BUZZ_CLI,
    '--workspace', REPO_ROOT
  ];

  const pyRes = spawnSync('python', pythonArgs, {
    env: ENV,
    cwd: REPO_ROOT,
    encoding: 'utf8'
  });

  if (pyRes.status !== 0) {
    error(`Proposal generation failed: ${pyRes.stderr || pyRes.stdout}`);
    fs.appendFileSync(FAILURE_LOG, `${new Date().toISOString()} PROPOSAL_FAILED ${pyRes.status}\n`);
    process.exit(pyRes.status || 1);
  }

  log('Proposal generated and posted successfully!');
  console.log(pyRes.stdout.trim());

  // Auto-sync artifacts to C:/Users/user/.buzz/ and knowledge base
  const userBuzz = 'C:/Users/user/.buzz';
  if (fs.existsSync(userBuzz)) {
    for (const dirName of ['PLANS', 'WORK_LOGS', 'GUIDES', 'OUTBOX']) {
      const srcDir = path.join(REPO_ROOT, dirName);
      const dstDir = path.join(userBuzz, dirName);
      if (fs.existsSync(srcDir)) {
        if (!fs.existsSync(dstDir)) fs.mkdirSync(dstDir, { recursive: true });
        fs.cpSync(srcDir, dstDir, { recursive: true, force: true });
        log(`Synced ${dirName} recursively to ${dstDir}`);
      }
    }
  }

  // Clear failure log on success
  if (fs.existsSync(FAILURE_LOG)) {
    fs.unlinkSync(FAILURE_LOG);
  }

  log('Daily Evolution Trigger completed with SUCCESS!');
}

main().catch(e => {
  error(`Unhandled error: ${e.message}`);
  process.exit(1);
});
