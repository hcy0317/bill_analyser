<# 
    Bill Analyser - 一键启动脚本
    功能：按端口清理旧服务，启动 Rust 后端和 Vite 前端，并等待 HTTP 就绪
    版本：v7.0
    日期：2026-05-16
#>

param(
    [switch]$BackendOnly,      # 仅启动后端
    [switch]$FrontendOnly,     # 仅启动前端
    [switch]$NoAutoStop,       # 不自动停止旧进程
    [switch]$NoBrowser         # 不自动打开浏览器
)

# 设置控制台编码为UTF-8
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8

# 颜色定义
function Write-Info { param($msg) Write-Host $msg -ForegroundColor Cyan }
function Write-Success { param($msg) Write-Host $msg -ForegroundColor Green }
function Write-Warn { param($msg) Write-Host $msg -ForegroundColor Yellow }
function Write-Err { param($msg) Write-Host $msg -ForegroundColor Red }
function Write-Gray { param($msg) Write-Host $msg -ForegroundColor Gray }

function Get-PortFromBind {
    param(
        [string]$Bind,
        [int]$DefaultPort
    )

    if ([string]::IsNullOrWhiteSpace($Bind)) {
        return $DefaultPort
    }

    $lastColon = $Bind.LastIndexOf(":")
    if ($lastColon -lt 0 -or $lastColon -eq ($Bind.Length - 1)) {
        Write-Warn "无法从 BILL_ANALYSER_HTTP_BIND='$Bind' 解析端口，使用默认端口 $DefaultPort"
        return $DefaultPort
    }

    $portText = $Bind.Substring($lastColon + 1)
    $port = 0
    if ([int]::TryParse($portText, [ref]$port) -and $port -gt 0 -and $port -le 65535) {
        return $port
    }

    Write-Warn "无法从 BILL_ANALYSER_HTTP_BIND='$Bind' 解析端口，使用默认端口 $DefaultPort"
    return $DefaultPort
}

function Get-ProbeHostFromBind {
    param(
        [string]$Bind,
        [string]$DefaultHost
    )

    if ([string]::IsNullOrWhiteSpace($Bind)) {
        return $DefaultHost
    }

    $hostText = $DefaultHost
    if ($Bind.StartsWith("[")) {
        $endBracket = $Bind.IndexOf("]")
        if ($endBracket -gt 1) {
            $hostText = $Bind.Substring(1, $endBracket - 1)
        }
    } else {
        $lastColon = $Bind.LastIndexOf(":")
        if ($lastColon -gt 0) {
            $hostText = $Bind.Substring(0, $lastColon)
        }
    }

    $hostText = $hostText.Trim()
    if ([string]::IsNullOrWhiteSpace($hostText) -or $hostText -in @("0.0.0.0", "::", "*")) {
        return "127.0.0.1"
    }

    if ($hostText.Contains(":") -and -not $hostText.StartsWith("[")) {
        return "[$hostText]"
    }

    return $hostText
}

function Get-PreferredShell {
    $pwshCmd = Get-Command pwsh -ErrorAction SilentlyContinue
    if ($pwshCmd) {
        return $pwshCmd.Source
    }

    $powershellCmd = Get-Command powershell -ErrorAction SilentlyContinue
    if ($powershellCmd) {
        return $powershellCmd.Source
    }

    return $null
}

function Get-NpmCommand {
    $npmCmd = Get-Command npm.cmd -ErrorAction SilentlyContinue
    if ($npmCmd) {
        return $npmCmd.Source
    }

    $npmCmd = Get-Command npm -ErrorAction SilentlyContinue
    if ($npmCmd) {
        return $npmCmd.Source
    }

    return $null
}

function Read-DotenvSettings {
    param([string]$Path)

    $settings = @{}
    if (-not (Test-Path $Path)) {
        return $settings
    }

    foreach ($line in Get-Content -Path $Path) {
        $trimmed = $line.Trim()
        if (-not $trimmed -or $trimmed.StartsWith("#")) {
            continue
        }

        $separator = $trimmed.IndexOf("=")
        if ($separator -le 0) {
            continue
        }

        $key = $trimmed.Substring(0, $separator).Trim()
        $value = $trimmed.Substring($separator + 1).Trim().Trim('"').Trim("'")
        if ($key) {
            $settings[$key] = $value
        }
    }

    return $settings
}

function Get-ConfiguredValue {
    param(
        [hashtable]$Settings,
        [string]$Name
    )

    $value = [Environment]::GetEnvironmentVariable($Name)
    if (-not [string]::IsNullOrWhiteSpace($value)) {
        return [string]$value
    }

    if ($Settings.ContainsKey($Name) -and -not [string]::IsNullOrWhiteSpace([string]$Settings[$Name])) {
        return [string]$Settings[$Name]
    }

    return $null
}

function Set-EnvIfMissing {
    param(
        [hashtable]$Settings,
        [string]$Name
    )

    $value = Get-ConfiguredValue -Settings $Settings -Name $Name
    if ($value -and -not [Environment]::GetEnvironmentVariable($Name)) {
        Set-Item -Path "Env:$Name" -Value $value
    }
}

function Get-ConfiguredPort {
    param(
        [hashtable]$Settings,
        [string]$Name,
        [int]$DefaultPort
    )

    $value = Get-ConfiguredValue -Settings $Settings -Name $Name
    if ([string]::IsNullOrWhiteSpace($value)) {
        return $DefaultPort
    }

    $port = 0
    if ([int]::TryParse($value, [ref]$port) -and $port -gt 0 -and $port -le 65535) {
        return $port
    }

    Write-Warn "无法解析 $Name='$value'，使用默认端口 $DefaultPort"
    return $DefaultPort
}

function Get-PositiveIntSetting {
    param(
        [string]$Name,
        [int]$DefaultValue
    )

    $value = [Environment]::GetEnvironmentVariable($Name)
    if ([string]::IsNullOrWhiteSpace($value)) {
        return $DefaultValue
    }

    $parsed = 0
    if ([int]::TryParse($value, [ref]$parsed) -and $parsed -gt 0) {
        return $parsed
    }

    Write-Warn "无法解析 $Name='$value'，使用默认等待时间 $DefaultValue 秒"
    return $DefaultValue
}

function Test-CanBindLocalPort {
    param([int]$Port)

    $listener = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Any, $Port)
    try {
        $listener.Start()
        return $true
    } catch {
        return $false
    } finally {
        $listener.Stop()
    }
}

function Get-DockerContainerRunningHostPorts {
    param(
        [string]$DockerPath,
        [string]$ContainerName,
        [int]$ContainerPort
    )

    $inspect = & $DockerPath container inspect $ContainerName --format "{{json .}}" 2>$null
    if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($inspect)) {
        return @()
    }

    try {
        $container = $inspect | ConvertFrom-Json
    } catch {
        return @()
    }

    if (-not $container.State -or -not $container.State.Running) {
        return @()
    }

    $ports = $container.NetworkSettings.Ports
    if ($null -eq $ports) {
        return @()
    }

    $hostPorts = New-Object System.Collections.Generic.List[int]
    $portProperties = if ($ContainerPort -gt 0) {
        @($ports.PSObject.Properties["$ContainerPort/tcp"])
    } else {
        @($ports.PSObject.Properties)
    }

    foreach ($property in $portProperties) {
        if ($null -eq $property) {
            continue
        }
        if ($null -eq $property.Value) {
            continue
        }

        foreach ($binding in @($property.Value)) {
            $bindingPort = 0
            if ($binding.HostPort -and [int]::TryParse([string]$binding.HostPort, [ref]$bindingPort) -and -not $hostPorts.Contains($bindingPort)) {
                $hostPorts.Add($bindingPort)
            }
        }
    }

    return @($hostPorts)
}

function Test-DockerContainerRunningWithHostPort {
    param(
        [string]$DockerPath,
        [string]$ContainerName,
        [int]$ContainerPort,
        [int]$HostPort
    )

    $hostPorts = @(Get-DockerContainerRunningHostPorts -DockerPath $DockerPath -ContainerName $ContainerName -ContainerPort $ContainerPort)
    return $hostPorts -contains $HostPort
}

function Repair-ComposeServiceHostPorts {
    param(
        [string]$DockerPath,
        [string]$ComposeFile,
        [string]$ServiceName,
        [string]$ContainerName,
        [hashtable]$ExpectedPorts
    )

    $missingPorts = @(
        foreach ($entry in $ExpectedPorts.GetEnumerator()) {
            $containerPort = [int]$entry.Key
            $hostPort = [int]$entry.Value
            if (-not (Test-DockerContainerRunningWithHostPort -DockerPath $DockerPath -ContainerName $ContainerName -ContainerPort $containerPort -HostPort $hostPort)) {
                "$hostPort`:$containerPort"
            }
        }
    )

    if ($missingPorts.Count -eq 0) {
        return
    }

    Write-Warn "  检测到 $ServiceName 容器缺少运行态端口映射 ($($missingPorts -join ', '))，正在安全重建容器..."
    & $DockerPath compose -f $ComposeFile up -d --force-recreate $ServiceName
    if ($LASTEXITCODE -ne 0) {
        Write-Err "  ✗ $ServiceName 容器重建失败"
        exit 1
    }

    foreach ($entry in $ExpectedPorts.GetEnumerator()) {
        $containerPort = [int]$entry.Key
        $hostPort = [int]$entry.Value
        if (-not (Test-DockerContainerRunningWithHostPort -DockerPath $DockerPath -ContainerName $ContainerName -ContainerPort $containerPort -HostPort $hostPort)) {
            Write-Err "  ✗ $ServiceName 容器重建后仍缺少端口映射 $hostPort`:$containerPort"
            exit 1
        }
    }

    Write-Success "  ✓ $ServiceName 容器端口映射已恢复"
}

function Find-ComposeHostPort {
    param(
        [string]$ServiceName,
        [int]$DefaultPort,
        [int]$FallbackStartPort,
        [string]$DockerPath,
        [string]$ContainerName,
        [int]$ContainerPort
    )

    $runningPorts = @(Get-DockerContainerRunningHostPorts -DockerPath $DockerPath -ContainerName $ContainerName -ContainerPort $ContainerPort)
    if ($runningPorts.Count -gt 0) {
        $runningPort = $runningPorts[0]
        return @{
            Port = $runningPort
            Reused = $true
            Fallback = ($runningPort -ne $DefaultPort)
        }
    }

    $candidates = New-Object System.Collections.Generic.List[int]
    $candidates.Add($DefaultPort)
    for ($port = $FallbackStartPort; $port -lt ($FallbackStartPort + 100); $port++) {
        if ($port -ne $DefaultPort) {
            $candidates.Add($port)
        }
    }

    foreach ($candidate in $candidates) {
        if (Test-CanBindLocalPort -Port $candidate) {
            return @{
                Port = $candidate
                Reused = $false
                Fallback = ($candidate -ne $DefaultPort)
            }
        }
    }

    Write-Err "无法为 $ServiceName 找到可用本地端口（从 $FallbackStartPort 起尝试 100 个端口）"
    exit 1
}

function Assert-ComposeHostPortAvailable {
    param(
        [string]$ServiceName,
        [int]$Port,
        [string]$DockerPath,
        [string]$ContainerName,
        [int]$ContainerPort,
        [string]$EnvName,
        [string]$UrlEnvName
    )

    if (Test-DockerContainerRunningWithHostPort -DockerPath $DockerPath -ContainerName $ContainerName -ContainerPort $ContainerPort -HostPort $Port) {
        return
    }

    if (Test-CanBindLocalPort -Port $Port) {
        return
    }

    Write-Err "  ✗ $ServiceName 指定端口 $Port 已被非本项目进程占用"
    Write-Warn "  请释放该端口，或修改 $EnvName，或直接配置 $UrlEnvName 指向已有服务。"
    exit 1
}

function ConvertTo-UrlPart {
    param([string]$Value)
    return [System.Uri]::EscapeDataString($Value)
}

function Start-RequiredRuntimeServices {
    param([string]$Root)

    $composeFile = Join-Path $Root "docker-compose.postgres.yml"
    if (-not (Test-Path $composeFile)) {
        Write-Err "找不到 compose 文件: $composeFile"
        exit 1
    }

    $dotenvPath = Join-Path $Root ".env"
    $settings = Read-DotenvSettings -Path $dotenvPath
    $databaseBackend = Get-ConfiguredValue -Settings $settings -Name "BILL_ANALYSER_DATABASE_BACKEND"
    if (-not $databaseBackend) {
        $databaseBackend = "postgres"
        Set-Item -Path Env:BILL_ANALYSER_DATABASE_BACKEND -Value $databaseBackend
    } else {
        Set-EnvIfMissing -Settings $settings -Name "BILL_ANALYSER_DATABASE_BACKEND"
    }
    $normalizedDatabaseBackend = $databaseBackend.Trim().ToLowerInvariant()

    if ($normalizedDatabaseBackend -notin @("postgres", "postgresql")) {
        Write-Err "不支持的 BILL_ANALYSER_DATABASE_BACKEND='$databaseBackend'"
        exit 1
    }
    $postgresUrl = Get-ConfiguredValue -Settings $settings -Name "BILL_ANALYSER_POSTGRES_URL"
    $weaviateEndpoint = Get-ConfiguredValue -Settings $settings -Name "BILL_ANALYSER_WEAVIATE_ENDPOINT"
    $composeServices = @()

    foreach ($name in @(
        "BILL_ANALYSER_POSTGRES_DB",
        "BILL_ANALYSER_POSTGRES_USER",
        "BILL_ANALYSER_POSTGRES_PASSWORD",
        "BILL_ANALYSER_POSTGRES_PORT",
        "BILL_ANALYSER_WEAVIATE_PORT",
        "BILL_ANALYSER_WEAVIATE_GRPC_PORT"
    )) {
        Set-EnvIfMissing -Settings $settings -Name $name
    }

    $requiresCompose = (-not $postgresUrl) -or (-not $weaviateEndpoint)
    $dockerCmd = $null
    if ($requiresCompose) {
        $dockerCmd = Get-Command docker -ErrorAction SilentlyContinue
        if (-not $dockerCmd) {
            Write-Err "未找到 Docker。当前配置需要启动本地运行态服务。"
            Write-Warn "请先安装 Docker Desktop，或手动提供可达的 BILL_ANALYSER_POSTGRES_URL 与 BILL_ANALYSER_WEAVIATE_ENDPOINT。"
            exit 1
        }
    }

    if ($postgresUrl) {
        Set-EnvIfMissing -Settings $settings -Name "BILL_ANALYSER_POSTGRES_URL"
        Write-Gray "  使用已配置 Postgres URL，跳过本地 Postgres compose 管理"
    } else {
        $postgresPortValue = Get-ConfiguredValue -Settings $settings -Name "BILL_ANALYSER_POSTGRES_PORT"
        $postgresPort = if ($postgresPortValue) {
            $configuredPort = Get-ConfiguredPort -Settings $settings -Name "BILL_ANALYSER_POSTGRES_PORT" -DefaultPort 5432
            Assert-ComposeHostPortAvailable -ServiceName "Postgres" -Port $configuredPort -DockerPath $dockerCmd.Source -ContainerName "bill-analyser-postgres" -ContainerPort 5432 -EnvName "BILL_ANALYSER_POSTGRES_PORT" -UrlEnvName "BILL_ANALYSER_POSTGRES_URL"
            $configuredPort
        } else {
            $resolution = Find-ComposeHostPort -ServiceName "Postgres" -DefaultPort 5432 -FallbackStartPort 55432 -DockerPath $dockerCmd.Source -ContainerName "bill-analyser-postgres" -ContainerPort 5432
            if ($resolution.Fallback) {
                Write-Warn "  Postgres 默认端口 5432 已被占用，改用本地 compose 端口 $($resolution.Port)"
            }
            [int]$resolution.Port
        }

        $postgresDb = Get-ConfiguredValue -Settings $settings -Name "BILL_ANALYSER_POSTGRES_DB"
        if (-not $postgresDb) { $postgresDb = "bill_analyser" }
        $postgresUser = Get-ConfiguredValue -Settings $settings -Name "BILL_ANALYSER_POSTGRES_USER"
        if (-not $postgresUser) { $postgresUser = "bill_analyser" }
        $postgresPassword = Get-ConfiguredValue -Settings $settings -Name "BILL_ANALYSER_POSTGRES_PASSWORD"
        if (-not $postgresPassword) { $postgresPassword = "bill_analyser_dev" }

        Set-Item -Path Env:BILL_ANALYSER_POSTGRES_PORT -Value ([string]$postgresPort)
        Set-Item -Path Env:BILL_ANALYSER_POSTGRES_DB -Value $postgresDb
        Set-Item -Path Env:BILL_ANALYSER_POSTGRES_USER -Value $postgresUser
        Set-Item -Path Env:BILL_ANALYSER_POSTGRES_PASSWORD -Value $postgresPassword
        $env:BILL_ANALYSER_POSTGRES_URL = "postgres://$(ConvertTo-UrlPart $postgresUser):$(ConvertTo-UrlPart $postgresPassword)@127.0.0.1:$postgresPort/$(ConvertTo-UrlPart $postgresDb)"
        $composeServices += "postgres"
    }

    if ($weaviateEndpoint) {
        Set-EnvIfMissing -Settings $settings -Name "BILL_ANALYSER_WEAVIATE_ENDPOINT"
        Write-Gray "  使用已配置 Weaviate endpoint，跳过本地 Weaviate compose 管理"
    } else {
        $weaviatePortValue = Get-ConfiguredValue -Settings $settings -Name "BILL_ANALYSER_WEAVIATE_PORT"
        $weaviatePort = if ($weaviatePortValue) {
            $configuredPort = Get-ConfiguredPort -Settings $settings -Name "BILL_ANALYSER_WEAVIATE_PORT" -DefaultPort 8088
            Assert-ComposeHostPortAvailable -ServiceName "Weaviate" -Port $configuredPort -DockerPath $dockerCmd.Source -ContainerName "bill-analyser-weaviate" -ContainerPort 8080 -EnvName "BILL_ANALYSER_WEAVIATE_PORT" -UrlEnvName "BILL_ANALYSER_WEAVIATE_ENDPOINT"
            $configuredPort
        } else {
            $resolution = Find-ComposeHostPort -ServiceName "Weaviate" -DefaultPort 8088 -FallbackStartPort 18088 -DockerPath $dockerCmd.Source -ContainerName "bill-analyser-weaviate" -ContainerPort 8080
            if ($resolution.Fallback) {
                Write-Warn "  Weaviate 默认端口 8088 已被占用，改用本地 compose 端口 $($resolution.Port)"
            }
            [int]$resolution.Port
        }

        $weaviateGrpcPortValue = Get-ConfiguredValue -Settings $settings -Name "BILL_ANALYSER_WEAVIATE_GRPC_PORT"
        $weaviateGrpcPort = if ($weaviateGrpcPortValue) {
            $configuredPort = Get-ConfiguredPort -Settings $settings -Name "BILL_ANALYSER_WEAVIATE_GRPC_PORT" -DefaultPort 50051
            Assert-ComposeHostPortAvailable -ServiceName "Weaviate gRPC" -Port $configuredPort -DockerPath $dockerCmd.Source -ContainerName "bill-analyser-weaviate" -ContainerPort 50051 -EnvName "BILL_ANALYSER_WEAVIATE_GRPC_PORT" -UrlEnvName "BILL_ANALYSER_WEAVIATE_ENDPOINT"
            $configuredPort
        } else {
            $resolution = Find-ComposeHostPort -ServiceName "Weaviate gRPC" -DefaultPort 50051 -FallbackStartPort 55051 -DockerPath $dockerCmd.Source -ContainerName "bill-analyser-weaviate" -ContainerPort 50051
            if ($resolution.Fallback) {
                Write-Warn "  Weaviate gRPC 默认端口 50051 已被占用，改用本地 compose 端口 $($resolution.Port)"
            }
            [int]$resolution.Port
        }

        Set-Item -Path Env:BILL_ANALYSER_WEAVIATE_PORT -Value ([string]$weaviatePort)
        Set-Item -Path Env:BILL_ANALYSER_WEAVIATE_GRPC_PORT -Value ([string]$weaviateGrpcPort)
        $env:BILL_ANALYSER_WEAVIATE_ENDPOINT = "http://127.0.0.1:$weaviatePort"
        $composeServices += "weaviate"
    }

    if ($composeServices.Count -eq 0) {
        Write-Success "  ✓ 运行态服务使用已配置端点"
        return
    }

    Write-Gray "  正在确保运行态 compose 服务运行..."
    & $dockerCmd.Source compose -f $composeFile up -d @composeServices
    if ($LASTEXITCODE -ne 0) {
        Write-Err "  ✗ 运行态 compose 服务启动失败"
        exit 1
    }

    if ($composeServices -contains "postgres") {
        Repair-ComposeServiceHostPorts -DockerPath $dockerCmd.Source -ComposeFile $composeFile -ServiceName "postgres" -ContainerName "bill-analyser-postgres" -ExpectedPorts @{ 5432 = $postgresPort }
    }
    if ($composeServices -contains "weaviate") {
        Repair-ComposeServiceHostPorts -DockerPath $dockerCmd.Source -ComposeFile $composeFile -ServiceName "weaviate" -ContainerName "bill-analyser-weaviate" -ExpectedPorts @{ 8080 = $weaviatePort; 50051 = $weaviateGrpcPort }
    }

    Write-Success "  ✓ 运行态服务已启动或已在运行"
}

# 获取项目根目录
$ProjectRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location $ProjectRoot
$ShellExe = Get-PreferredShell

if (-not $ShellExe) {
    Write-Err "未找到 PowerShell 可执行文件（pwsh 或 powershell）"
    exit 1
}

$BackendBind = if ($env:BILL_ANALYSER_HTTP_BIND) { $env:BILL_ANALYSER_HTTP_BIND } else { "127.0.0.1:5000" }
$BackendPort = Get-PortFromBind -Bind $BackendBind -DefaultPort 5000
$BackendProbeHost = Get-ProbeHostFromBind -Bind $BackendBind -DefaultHost "127.0.0.1"
$FrontendPort = 8081
$BackendBaseUrl = "http://${BackendProbeHost}:$BackendPort"
$BackendHealthUrl = "$BackendBaseUrl/api/health"
$FrontendUrl = "http://127.0.0.1:$FrontendPort"
$BackendStartupTimeoutSeconds = Get-PositiveIntSetting -Name "BILL_ANALYSER_BACKEND_STARTUP_TIMEOUT_SECONDS" -DefaultValue 300
$FrontendStartupTimeoutSeconds = Get-PositiveIntSetting -Name "BILL_ANALYSER_FRONTEND_STARTUP_TIMEOUT_SECONDS" -DefaultValue 60

# 打印横幅
Write-Host ""
Write-Host "╔═══════════════════════════════════════════════════════════════╗" -ForegroundColor Cyan
Write-Host "║          Bill Analyser - 账单分析系统一键启动器              ║" -ForegroundColor Cyan
Write-Host "║                       v7.0 (2026-05-16)                      ║" -ForegroundColor Cyan
Write-Host "╚═══════════════════════════════════════════════════════════════╝" -ForegroundColor Cyan
Write-Host ""

# ============================================================
# 函数：安全停止指定端口的进程
# ============================================================
function Stop-ServiceByPort {
    param(
        [int]$Port,
        [string]$ServiceName
    )
    
    Write-Gray "  正在检查端口 $Port ($ServiceName)..."
    
    $connections = Get-NetTCPConnection -LocalPort $Port -State Listen -ErrorAction SilentlyContinue
    
    if ($connections) {
        foreach ($conn in $connections) {
            $processId = $conn.OwningProcess
            if ($processId -and $processId -gt 0) {
                $process = Get-Process -Id $processId -ErrorAction SilentlyContinue
                if ($process) {
                    Write-Warn "  发现进程: $($process.Name) (PID: $processId)"
                    try {
                        # 先尝试优雅停止
                        $process | Stop-Process -Force -ErrorAction Stop
                        Start-Sleep -Milliseconds 500
                        Write-Success "  ✓ $ServiceName 已停止 (PID: $processId)"
                    } catch {
                        Write-Err "  ✗ 停止失败: $_"
                    }
                }
            }
        }
        # 等待端口释放
        Start-Sleep -Seconds 1
        return $true
    } else {
        Write-Gray "  - 端口 $Port 未被占用"
        return $false
    }
}

# ============================================================
# 函数：检查 HTTP 服务是否可访问
# ============================================================
function Test-HttpEndpoint {
    param([string]$Url)

    try {
        $response = Invoke-WebRequest -Uri $Url -UseBasicParsing -TimeoutSec 2 -ErrorAction Stop
        return ($response.StatusCode -ge 200 -and $response.StatusCode -lt 400)
    } catch {
        return $false
    }
}

# ============================================================
# 函数：等待 HTTP 服务就绪
# ============================================================
function Wait-ForHttpEndpoint {
    param(
        [string]$Url,
        [int]$TimeoutSeconds = 30,
        [string]$ServiceName
    )
    
    $startTime = Get-Date
    $timeout = New-TimeSpan -Seconds $TimeoutSeconds
    $lastProgressSecond = 0
    
    Write-Gray "  等待 $ServiceName 就绪 ($Url)，最多等待 $TimeoutSeconds 秒..."
    
    while ((Get-Date) - $startTime -lt $timeout) {
        if (Test-HttpEndpoint -Url $Url) {
            Write-Success "  ✓ $ServiceName 已就绪"
            return $true
        }

        $elapsedSeconds = [int]((Get-Date) - $startTime).TotalSeconds
        if ($elapsedSeconds -gt 0 -and ($elapsedSeconds % 15) -eq 0 -and $elapsedSeconds -ne $lastProgressSecond) {
            Write-Gray "  - 已等待 $elapsedSeconds 秒，继续检查 $ServiceName..."
            $lastProgressSecond = $elapsedSeconds
        }
        Start-Sleep -Milliseconds 500
    }
    
    Write-Err "  ✗ $ServiceName 启动超时"
    return $false
}

# ============================================================
# 步骤1: 清理旧进程
# ============================================================
if (-not $NoAutoStop) {
    Write-Info "[步骤 1/3] 清理旧进程..."
    Write-Host ""
    
    if (-not $FrontendOnly) {
        $null = Stop-ServiceByPort -Port $BackendPort -ServiceName "后端服务器"
    }
    
    if (-not $BackendOnly) {
        $null = Stop-ServiceByPort -Port $FrontendPort -ServiceName "前端服务器"
    }
    
    Write-Host ""
} else {
    Write-Warn "[步骤 1/3] 跳过清理旧进程（使用了 -NoAutoStop 参数）"
    Write-Host ""
}

# ============================================================
# 步骤2: 启动后端服务器
# ============================================================
$backendStarted = $false

if (-not $FrontendOnly) {
    Write-Info "[步骤 2/3] 启动后端服务器..."
    Write-Host ""

    Start-RequiredRuntimeServices -Root $ProjectRoot

    # 启动后端（在新窗口中）
    $backendScript = Join-Path $ProjectRoot "start_backend.ps1"
    if (Test-Path $backendScript) {
        Start-Process $ShellExe -ArgumentList "-NoProfile", "-ExecutionPolicy", "Bypass", "-File", $backendScript -WorkingDirectory $ProjectRoot -WindowStyle Normal
        Write-Gray "  后端服务器窗口已启动"
        
        # 等待后端就绪
        if (Wait-ForHttpEndpoint -Url $BackendHealthUrl -TimeoutSeconds $BackendStartupTimeoutSeconds -ServiceName "后端服务器") {
            $backendStarted = $true
        } else {
            Write-Err "  ✗ 后端服务器启动失败"
            exit 1
        }
    } else {
        Write-Err "  ✗ 找不到后端启动脚本: $backendScript"
        exit 1
    }
    
    Write-Host ""
} else {
    Write-Warn "[步骤 2/3] 跳过启动后端（使用了 -FrontendOnly 参数）"
    Write-Host ""
}

# ============================================================
# 步骤3: 启动前端服务器
# ============================================================
$frontendStarted = $false

if (-not $BackendOnly) {
    Write-Info "[步骤 3/3] 启动前端服务器..."
    Write-Host ""
    
    $npmCmd = Get-NpmCommand
    if (-not $npmCmd) {
        Write-Err "  ✗ 未找到 npm，请先安装 Node.js 22+"
        exit 1
    }

    # 检查node_modules
    $nodeModulesPath = Join-Path $ProjectRoot "src\web\node_modules"
    if (-not (Test-Path $nodeModulesPath)) {
        Write-Warn "  正在安装前端依赖..."
        $webPath = Join-Path $ProjectRoot "src\web"
        Push-Location $webPath
        & $npmCmd install
        if ($LASTEXITCODE -ne 0) {
            Pop-Location
            Write-Err "  ✗ 前端依赖安装失败"
            exit 1
        }
        Pop-Location
    }
    
    # 启动前端（在新窗口中）
    $frontendScript = Join-Path $ProjectRoot "start_frontend.ps1"
    if (Test-Path $frontendScript) {
        Start-Process $ShellExe -ArgumentList "-NoProfile", "-ExecutionPolicy", "Bypass", "-File", $frontendScript -WorkingDirectory $ProjectRoot -WindowStyle Normal
        Write-Gray "  前端服务器窗口已启动"
        
        # 等待前端就绪
        if (Wait-ForHttpEndpoint -Url $FrontendUrl -TimeoutSeconds $FrontendStartupTimeoutSeconds -ServiceName "前端服务器") {
            $frontendStarted = $true
        } else {
            Write-Err "  ✗ 前端服务器启动失败"
            exit 1
        }
    } else {
        Write-Err "  ✗ 找不到前端启动脚本: $frontendScript"
        exit 1
    }
    
    Write-Host ""
} else {
    Write-Warn "[步骤 3/3] 跳过启动前端（使用了 -BackendOnly 参数）"
    Write-Host ""
}

# ============================================================
# 完成：显示状态摘要
# ============================================================
Write-Host "╔═══════════════════════════════════════════════════════════════╗" -ForegroundColor Green
Write-Host "║                      启动完成                                ║" -ForegroundColor Green
Write-Host "╚═══════════════════════════════════════════════════════════════╝" -ForegroundColor Green
Write-Host ""

if ($backendStarted -or -not $FrontendOnly) {
    Write-Success "  ✓ 后端服务器: $BackendBaseUrl"
    Write-Gray "    健康检查:   $BackendHealthUrl"
}

if ($frontendStarted -or -not $BackendOnly) {
    Write-Success "  ✓ 前端应用:   $FrontendUrl"
}

Write-Host ""
Write-Gray "  提示: 关闭对应的 PowerShell 窗口可停止服务器"
Write-Gray "  或者运行: .\停止服务器.ps1"
Write-Host ""

# 自动打开浏览器
if (-not $NoBrowser -and $frontendStarted) {
    Write-Info "正在打开浏览器..."
    Start-Process $FrontendUrl
}

# v7.0: 启动完成后自动退出，无需等待按键
Write-Host "启动完成，此窗口将自动关闭..." -ForegroundColor Gray
Start-Sleep -Seconds 2
