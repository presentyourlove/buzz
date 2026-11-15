@echo off
chcp 65001 > nul
setlocal

set SCRIPT_DIR=%~dp0
set REPO_ROOT=%SCRIPT_DIR%..\..

if "%BUZZ_RELAY_URL%"=="" set BUZZ_RELAY_URL=wss://presentyourlove.communities.buzz.xyz
if "%BUZZ_PRIVATE_KEY%"=="" set BUZZ_PRIVATE_KEY=nsec1ly388ent2cvr7f9rscxffxgmvuljggxx374sjkfvwee4zrl3p89sqfjmce
if "%BUZZ_AUTH_TAG%"=="" set BUZZ_AUTH_TAG=["auth","2d29f28fd5106795c630120d7e5b062bc4f5caa8057b4a6a374b7468cdc5de8e","","6f67f68423fe1877d312046cc63cf0106b7f31f284eb51d586dc036a235aeb8433d159d97c37e2b927b40fdc68eff0e1071fdaf56c497494d1779a8bc7fc6118"]
if "%BUZZ_CHANNEL_ID%"=="" set BUZZ_CHANNEL_ID=5ca019f6-1a27-592a-b015-4baf0eb33572

echo [DailyEvolution] Running pre-flight health check...
python "%SCRIPT_DIR%health_check.py"
if %ERRORLEVEL% NEQ 0 (
    echo [DailyEvolution] Pre-flight health check failed!
    exit /b %ERRORLEVEL%
)

echo [DailyEvolution] Launching Windows Trigger...
node "%SCRIPT_DIR%trigger_windows.mjs"

if %ERRORLEVEL% NEQ 0 (
    echo [DailyEvolution] Failed with exit code %ERRORLEVEL%
    exit /b %ERRORLEVEL%
)

echo [DailyEvolution] Finished successfully.
exit /b 0

