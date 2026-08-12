@echo off
setlocal EnableDelayedExpansion
chcp 65001 >nul
title Bill Analyser - Launcher

set "LAUNCHER_PS1=%~dpn0.ps1"
set "DEV_MANIFEST=%~dp0.git\ai\dev-services\services.manifest.json"

cd /d "%~dp0"

REM Use ASCII-only output here. cmd.exe can misparse UTF-8 box drawing
REM characters before PowerShell takes over.
set "PWSH_EXE=%ProgramFiles%\PowerShell\7\pwsh.exe"
if exist "%PWSH_EXE%" goto launch

set "PWSH_EXE="
for /f "delims=" %%I in ('where pwsh 2^>nul') do if not defined PWSH_EXE set "PWSH_EXE=%%I"
if defined PWSH_EXE goto launch

echo [error] PowerShell 7 ^(pwsh^) was not found. Please install PowerShell 7.
pause
exit /b 1

:launch
echo [start] Using PowerShell 7: %PWSH_EXE%
"%PWSH_EXE%" -NoProfile -ExecutionPolicy Bypass -File "%LAUNCHER_PS1%" -Headless -ProcessManifestPath "%DEV_MANIFEST%" %*
exit /b !ERRORLEVEL!
