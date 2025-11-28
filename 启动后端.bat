@echo off
chcp 65001 >nul
echo ========================================
echo 账单分析系统 - 后端启动器
echo ========================================
echo.

cd /d "%~dp0"

REM 检查是否已经在运行 (精确匹配端口5000)
netstat -ano | findstr "LISTENING" | findstr ":5000 " >NUL
if "%ERRORLEVEL%"=="0" (
    echo [警告] 端口5000已被占用
    echo.
    echo 正在尝试停止旧进程...
    for /f "tokens=5" %%a in ('netstat -ano ^| findstr "LISTENING" ^| findstr ":5000 "') do (
        echo 停止进程 PID: %%a
        taskkill /F /PID %%a >nul 2>&1
    )
    timeout /t 2 >NUL
    echo 已清理端口，继续启动...
    echo.
)

REM 启动后端
echo [启动] 正在启动后端服务器...
start "Bill Analyser Backend" powershell -ExecutionPolicy Bypass -NoExit -File "%~dp0start_backend.ps1"

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
