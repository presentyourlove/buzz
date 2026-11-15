@echo off
chcp 65001 > nul
echo ========================================================
echo  Buzz 第二大腦 (Obsidian) 背景常駐守護停止器
echo ========================================================

powershell -NoProfile -Command "$procs = Get-CimInstance Win32_Process | Where-Object { $_.Name -eq 'node.exe' -and $_.CommandLine -like '*main.mjs --daemon*' }; if ($procs) { foreach ($p in $procs) { Stop-Process -Id $p.ProcessId -Force; Write-Host '✔ 已安全停止背景守護程序 (PID:' $p.ProcessId ')' -ForegroundColor Green } } else { Write-Host 'ℹ 目前沒有正在運行的背景守護程序。' -ForegroundColor Yellow }"

echo ========================================================
pause
