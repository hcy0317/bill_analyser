@echo off
setlocal EnableDelayedExpansion
chcp 65001 >nul
title Bill Analyser - Launcher

set "LAUNCHER_PS1=%~dpn0.ps1"

cd /d "%~dp0"

REM Use ASCII-only output here. cmd.exe can misparse UTF-8 box drawing
REM characters before PowerShell takes over.
where pwsh >nul 2>&1
if not errorlevel 1 (
    echo [start] Using PowerShell Core...
    pwsh -NoProfile -ExecutionPolicy Bypass -File "%LAUNCHER_PS1%" %*
    exit /b !ERRORLEVEL!
) else (
    where powershell >nul 2>&1
    if not errorlevel 1 (
        echo [start] Using Windows PowerShell...
        powershell -NoProfile -ExecutionPolicy Bypass -File "%LAUNCHER_PS1%" %*
        exit /b !ERRORLEVEL!
    ) else (
        echo [error] PowerShell was not found. Please install PowerShell.
        pause
        exit /b 1
    )
)
