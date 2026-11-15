@echo off
chcp 65001 > nul
echo ========================================================
echo  Buzz 第二大腦 (Obsidian) 背景常駐守護狀態檢查
echo ========================================================

cd /d "C:\Users\user\.buzz\PLANS\sync_obsidian"

powershell -NoProfile -Command "$p = Get-CimInstance Win32_Process | Where-Object { $_.Name -eq 'node.exe' -and $_.CommandLine -like '*main.mjs --daemon*' }; if ($p) { Write-Host '🟢 守護程序運行中' -ForegroundColor Green; Write-Host '   行程代號 (PID):' $p.ProcessId; Write-Host '   啟動時間:' $p.CreationDate } else { Write-Host '🔴 守護程序目前未運行' -ForegroundColor Yellow }"

echo --------------------------------------------------------
echo 最新心跳與同步紀錄：
if exist "C:\私人\ai agent\buzz-obsidian\sync-meta\heartbeat.json" (
    powershell -NoProfile -Command "Get-Content 'C:\私人\ai agent\buzz-obsidian\sync-meta\heartbeat.json' | ConvertFrom-Json | Select-Object timestamp, last_sync_status, daemon_uptime_seconds | Format-List"
) else (
    echo (尚無心跳檔案)
)

echo ========================================================
pause
