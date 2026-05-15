@echo off
chcp 65001 >nul
echo ========================================
echo Stop Bill Analyser Servers
echo ========================================
echo.

cd /d "%~dp0"

echo [1/2] Checking backend server (port 5000)
set FOUND_BACKEND=0
for /f "tokens=5" %%a in ('netstat -ano ^| findstr ":5000.*LISTENING" 2^>nul') do (
    if %%a GTR 0 (
        echo Found backend PID: %%a
        taskkill /F /PID %%a >nul 2>&1
        if not errorlevel 1 (
            echo Backend stopped
            set FOUND_BACKEND=1
        )
    )
)
if "%FOUND_BACKEND%"=="0" echo Backend not found

echo.

echo [2/2] Checking frontend server (port 8081)
set FOUND_FRONTEND=0
for /f "tokens=5" %%a in ('netstat -ano ^| findstr ":8081.*LISTENING" 2^>nul') do (
    if %%a GTR 0 (
        echo Found frontend PID: %%a
        taskkill /F /PID %%a >nul 2>&1
        if not errorlevel 1 (
            echo Frontend stopped
            set FOUND_FRONTEND=1
        )
    )
)
if "%FOUND_FRONTEND%"=="0" echo Frontend not found

echo.
echo ========================================
echo Done
echo ========================================

pause
