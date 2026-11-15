@echo off
chcp 65001 > nul
echo ========================================================
echo  Buzz 第二大腦 (Obsidian) 背景常駐守護啟動器
echo ========================================================

cd /d "C:\Users\user\.buzz\PLANS\sync_obsidian"

:: 檢查是否已有相同守護行程在運行
powershell -NoProfile -Command "$p = Get-CimInstance Win32_Process | Where-Object { $_.Name -eq 'node.exe' -and $_.CommandLine -like '*main.mjs --daemon*' }; if ($p) { Write-Host 'ℹ 守護程序已在背景運行中 (PID:' $p.ProcessId ')，無需重複啟動！' -ForegroundColor Yellow; exit 100 }"
if %ERRORLEVEL% EQU 100 goto end

:: 啟動隱藏視窗常駐守護
powershell -NoProfile -Command "Start-Process -FilePath 'node' -ArgumentList 'main.mjs --daemon' -WorkingDirectory 'C:\Users\user\.buzz\PLANS\sync_obsidian' -WindowStyle Hidden"

echo [資訊] ✔ 背景守護程序已成功啟動！
echo 守護模式：每 30 分鐘自動提煉對話、歸檔冷記憶並自動推送 GitHub
echo 完工通報：每次同步完成後將自動發送 Windows 桌面 Toast 橫幅通知
echo --------------------------------------------------------
echo 提示：您可以隨時執行 status-daemon.bat 檢視運行狀態
echo ========================================================

:end
pause
