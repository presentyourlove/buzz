const { spawn } = require('node:child_process');
const path = require('node:path');
const fs = require('node:fs');

const logFile = path.resolve(__dirname, 'daemon.log');
const out = fs.openSync(logFile, 'a');
const err = fs.openSync(logFile, 'a');

const child = spawn(process.execPath, [path.resolve(__dirname, 'main.mjs'), '--daemon'], {
  detached: true,
  stdio: ['ignore', out, err],
  windowsHide: true,
  cwd: __dirname,
});

child.unref();
console.log(`✔ 背景守護程序已成功生成！PID: ${child.pid}`);
process.exit(0);
