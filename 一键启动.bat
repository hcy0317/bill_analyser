@echo off
chcp 65001 >nul
title Bill Analyser - 一键启动

echo.
echo ╔═══════════════════════════════════════════════════════════════╗
echo ║          Bill Analyser - 账单分析系统一键启动器              ║
echo ╚═══════════════════════════════════════════════════════════════╝
echo.

cd /d "%~dp0"

REM 检查PowerShell是否可用
where pwsh >nul 2>&1
if %ERRORLEVEL%==0 (
    echo [启动] 使用 PowerShell Core 启动...
    pwsh -ExecutionPolicy Bypass -File "%~dp0一键启动.ps1"
) else (
    where powershell >nul 2>&1
    if %ERRORLEVEL%==0 (
        echo [启动] 使用 Windows PowerShell 启动...
        powershell -ExecutionPolicy Bypass -File "%~dp0一键启动.ps1"
    ) else (
        echo [错误] 未找到 PowerShell，请安装 PowerShell
        pause
        exit /b 1
    )
)
