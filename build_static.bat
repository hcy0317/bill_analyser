@echo off
chcp 65001 >nul
echo ========================================
echo Build Frontend Static Assets
echo ========================================
echo.

cd /d "%~dp0"

where pwsh >nul 2>&1
if %ERRORLEVEL%==0 (
    echo Using PowerShell Core...
    pwsh -ExecutionPolicy Bypass -File "%~dp0build_static.ps1"
) else (
    where powershell >nul 2>&1
    if %ERRORLEVEL%==0 (
        echo Using Windows PowerShell...
        powershell -ExecutionPolicy Bypass -File "%~dp0build_static.ps1"
    ) else (
        echo Error: PowerShell not found.
        pause
        exit /b 1
    )
)

pause
