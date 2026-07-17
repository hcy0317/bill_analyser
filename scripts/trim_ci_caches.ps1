param(
    [int]$CargoCacheLimitMb = 450,
    [int]$NpmCacheLimitMb = 300,
    [switch]$CiExact
)

$ErrorActionPreference = "Stop"
$repoRoot = Split-Path -Parent $PSScriptRoot

function Get-PathSizeMb {
    param([string[]]$Paths)
    $total = 0L
    foreach ($path in $Paths) {
        if (Test-Path -LiteralPath $path) {
            $size = (Get-ChildItem -LiteralPath $path -Recurse -Force -ErrorAction SilentlyContinue |
                Measure-Object -Property Length -Sum).Sum
            if ($null -ne $size) {
                $total += [int64]$size
            }
        }
    }
    return [math]::Ceiling($total / 1MB)
}

function Remove-IfExists {
    param(
        [string[]]$Paths,
        [Parameter(Mandatory = $true)][string]$AllowedRoot
    )
    $resolvedAllowedRoot = [System.IO.Path]::GetFullPath($AllowedRoot).TrimEnd([System.IO.Path]::DirectorySeparatorChar)
    foreach ($path in $Paths) {
        if (Test-Path -LiteralPath $path) {
            $resolvedPath = [System.IO.Path]::GetFullPath((Resolve-Path -LiteralPath $path).Path)
            if (-not ($resolvedPath -eq $resolvedAllowedRoot -or $resolvedPath.StartsWith("$resolvedAllowedRoot$([System.IO.Path]::DirectorySeparatorChar)"))) {
                throw "Refusing to remove path outside allowed root: $resolvedPath"
            }
            Remove-Item -LiteralPath $resolvedPath -Recurse -Force -ErrorAction SilentlyContinue
        }
    }
}

Remove-IfExists @(
    (Join-Path $repoRoot "target"),
    (Join-Path $repoRoot "workspace.lcov"),
    (Join-Path $repoRoot "coverage.json"),
    (Join-Path $repoRoot ".coverage"),
    (Join-Path $repoRoot "src\web\coverage"),
    (Join-Path $repoRoot "src\web\dist")
) -AllowedRoot $repoRoot
Get-ChildItem -LiteralPath $repoRoot -Force -Filter ".coverage.*" -ErrorAction SilentlyContinue |
    Remove-Item -Force -ErrorAction SilentlyContinue

$cargoHome = if ($env:CARGO_HOME) { $env:CARGO_HOME } else { Join-Path $HOME ".cargo" }
$cargoCacheMb = Get-PathSizeMb @(
    (Join-Path $cargoHome "registry"),
    (Join-Path $cargoHome "git"),
    (Join-Path $cargoHome "bin\cargo-llvm-cov.exe")
)
if ($cargoCacheMb -gt $CargoCacheLimitMb) {
    Remove-IfExists @(
        (Join-Path $cargoHome "registry\src"),
        (Join-Path $cargoHome "git\checkouts")
    ) -AllowedRoot $cargoHome
}

$npmCache = npm config get cache
$npmCacheMb = Get-PathSizeMb @($npmCache)
if ($npmCacheMb -gt $NpmCacheLimitMb) {
    npm cache clean --force
}

if ($CiExact) {
    Remove-IfExists @((Join-Path $repoRoot "src\web\node_modules")) -AllowedRoot $repoRoot
}
