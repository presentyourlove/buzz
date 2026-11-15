@echo off
chcp 65001 > nul
echo ========================================================
echo  註冊 Windows 開機/登入自動常駐守護 (Wax Daemon)
echo ========================================================

powershell -NoProfile -Command "$ws = New-Object -ComObject WScript.Shell; $startup = [Environment]::GetFolderPath('Startup'); $s = $ws.CreateShortcut(\"$startup\\BuzzObsidianSyncDaemon.lnk\"); $s.TargetPath = 'powershell.exe'; $s.Arguments = '-NoProfile -WindowStyle Hidden -Command \"Start-Process -FilePath ''node'' -ArgumentList ''main.mjs --daemon'' -WorkingDirectory ''C:\\Users\\user\\.buzz\\PLANS\\sync_obsidian'' -WindowStyle Hidden\"'; $s.WorkingDirectory = 'C:\\Users\\user\\.buzz\\PLANS\\sync_obsidian'; $s.Description = 'Buzz Obsidian Auto Sync Daemon'; $s.Save(); Write-Host '✔ 成功註冊至 Windows Startup 啟動目錄！' -ForegroundColor Green; Write-Host '捷徑路徑:' \"$startup\\BuzzObsidianSyncDaemon.lnk\""

echo --------------------------------------------------------
echo 現在每次您登入 Windows，系統將自動於背景靜默啟動記憶守護程序！
echo ========================================================
pause
