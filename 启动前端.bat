@echo off
chcp 65001 >nul
echo ========================================
echo 账单分析系统 - 前端启动器
echo ========================================
echo.

cd /d "%~dp0"

where pwsh >nul 2>&1
if %ERRORLEVEL%==0 (
    echo [启动] 使用 PowerShell Core 启动前端...
    start "Bill Analyser Frontend" pwsh -ExecutionPolicy Bypass -NoExit -File "%~dp0start_frontend.ps1"
) else (
    where powershell >nul 2>&1
    if %ERRORLEVEL%==0 (
        echo [启动] 使用 Windows PowerShell 启动前端...
        start "Bill Analyser Frontend" powershell -ExecutionPolicy Bypass -NoExit -File "%~dp0start_frontend.ps1"
    ) else (
        echo [错误] 未找到 PowerShell，请安装 PowerShell
        pause
        exit /b 1
    )
)

REM 等待启动
timeout /t 5 >NUL

REM 检查端口
netstat -ano | findstr ":8081" >NUL
if "%ERRORLEVEL%"=="0" (
    echo.
    echo ========================================
    echo 前端已成功启动
    echo ========================================
    echo.
    echo 访问地址: http://localhost:8081
    echo.
    echo 提示: 关闭PowerShell窗口可停止服务器
    echo ========================================
) else (
    echo.
    echo [错误] 前端启动失败
    echo 请查看PowerShell窗口的错误信息
)

pause
