@echo off
setlocal
chcp 65001 >nul
title Bill Analyser - Stop Services

set "DEV_SCRIPT=%~dp0scripts\dev.ps1"

cd /d "%~dp0"

REM Keep this launcher ASCII-only until PowerShell takes over.
set "PWSH_EXE=%ProgramFiles%\PowerShell\7\pwsh.exe"
if exist "%PWSH_EXE%" goto stop_services

set "PWSH_EXE="
for /f "delims=" %%I in ('where pwsh 2^>nul') do if not defined PWSH_EXE set "PWSH_EXE=%%I"
if defined PWSH_EXE goto stop_services

echo [error] PowerShell 7 ^(pwsh^) was not found. Please install PowerShell 7.
pause
exit /b 1

:stop_services
if not exist "%DEV_SCRIPT%" (
    echo [error] Managed stop script was not found: %DEV_SCRIPT%
    pause
    exit /b 1
)

echo [stop] Stopping Bill Analyser backend and frontend...
"%PWSH_EXE%" -NoProfile -ExecutionPolicy Bypass -File "%DEV_SCRIPT%" stop
set "EXIT_CODE=%ERRORLEVEL%"

if "%EXIT_CODE%"=="0" (
    echo [done] Backend and frontend stopped. PostgreSQL and Weaviate are still running.
) else (
    echo [error] Failed to stop managed services. Exit code: %EXIT_CODE%
)

pause
exit /b %EXIT_CODE%
