@echo off
chcp 65001 >nul
echo ========================================
echo 账单分析系统 - 后端启动器
echo ========================================
echo.

cd /d "%~dp0"

where pwsh >nul 2>&1
if %ERRORLEVEL%==0 (
    echo [启动] 使用 PowerShell Core 启动后端...
    start "Bill Analyser Backend" pwsh -ExecutionPolicy Bypass -NoExit -File "%~dp0start_backend.ps1"
) else (
    where powershell >nul 2>&1
    if %ERRORLEVEL%==0 (
        echo [启动] 使用 Windows PowerShell 启动后端...
        start "Bill Analyser Backend" powershell -ExecutionPolicy Bypass -NoExit -File "%~dp0start_backend.ps1"
    ) else (
        echo [错误] 未找到 PowerShell，请安装 PowerShell
        pause
        exit /b 1
    )
)

REM 等待启动
timeout /t 3 >NUL

REM 检查端口
netstat -ano | findstr ":5000" >NUL
if "%ERRORLEVEL%"=="0" (
    echo.
    echo ========================================
    echo 后端已成功启动
    echo ========================================
    echo.
    echo 后端API: http://localhost:5000
    echo 健康检查: http://localhost:5000/api/health
    echo.
    echo 提示: 关闭PowerShell窗口可停止服务器
    echo ========================================
) else (
    echo.
    echo [错误] 后端启动失败
    echo 请查看PowerShell窗口的错误信息
)

pause
