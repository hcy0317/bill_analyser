@echo off
chcp 65001 >nul
echo ========================================
echo 账单分析系统 - 前端启动器
echo ========================================
echo.

cd /d "%~dp0"

REM 检查是否已经在运行
netstat -ano | findstr "LISTENING" | findstr ":8081" >NUL
if "%ERRORLEVEL%"=="0" (
    echo [警告] 端口8081已被占用
    echo 请先运行 停止服务器.bat
    pause
    exit /b 1
)

REM 启动前端
echo [启动] 正在启动前端服务器...
start "Bill Analyser Frontend" powershell -ExecutionPolicy Bypass -NoExit -File "%~dp0start_frontend.ps1"

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
