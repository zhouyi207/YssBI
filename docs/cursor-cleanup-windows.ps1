# Cursor 缓存清理脚本（Windows）
# 用途：清理膨胀的 globalStorage / workspaceStorage / worktrees，缓解 Cursor 卡顿。
#
# 用法（PowerShell）：
#   1. 完全退出 Cursor（文件 → 退出，或托盘图标 → Quit）
#   2. 以普通用户打开 PowerShell
#   3. 执行：
#        Set-ExecutionPolicy -Scope Process -ExecutionPolicy Bypass
#        & "D:\Desktop\YssBI\docs\cursor-cleanup-windows.ps1"
#
# 可选参数：
#   -ProjectName "YssBI"          匹配 workspace 名称（默认 YssBI）
#   -ProjectPath "D:\Desktop\YssBI"  精确匹配 folder 路径（默认本项目路径）
#   -ForceStopCursor               自动结束 Cursor 进程（默认需手动退出）

param(
    [string]$ProjectName = "YssBI",
    [string]$ProjectPath = "D:\Desktop\YssBI",
    [switch]$ForceStopCursor
)

$ErrorActionPreference = "Stop"

$globalStorage = Join-Path $env:APPDATA "Cursor\User\globalStorage"
$workspaceStorage = Join-Path $env:APPDATA "Cursor\User\workspaceStorage"
$worktreesRoot = Join-Path $env:USERPROFILE ".cursor\worktrees"

function Write-Step([string]$Message) {
    Write-Host ""
    Write-Host "==> $Message" -ForegroundColor Cyan
}

function Test-CursorRunning {
    return [bool](Get-Process -Name "Cursor*" -ErrorAction SilentlyContinue)
}

function Remove-IfExists([string]$Path) {
    if (-not (Test-Path $Path)) {
        Write-Host "  skip (not found): $Path"
        return
    }
    Remove-Item -Path $Path -Recurse -Force -ErrorAction Stop
    Write-Host "  removed: $Path"
}

Write-Step "1. 检查 Cursor 是否已退出"
if (Test-CursorRunning) {
    if ($ForceStopCursor) {
        Write-Host "  正在结束 Cursor 进程..."
        Get-Process -Name "Cursor*" -ErrorAction SilentlyContinue | Stop-Process -Force
        Start-Sleep -Seconds 3
    }
    if (Test-CursorRunning) {
        Write-Host "  请先完全退出 Cursor，或使用 -ForceStopCursor 参数。" -ForegroundColor Yellow
        exit 1
    }
}
Write-Host "  OK"

Write-Step "2. 删除 globalStorage（重建全局数据库）"
$globalFiles = @(
    "storage.json",
    "state.vscdb",
    "state.vscdb-shm",
    "state.vscdb-wal",
    "state.vscdb.backup",
    "state.vscdb.options.json"
)
foreach ($name in $globalFiles) {
    Remove-IfExists (Join-Path $globalStorage $name)
}

Write-Step "3. 删除指向当前项目的 workspaceStorage"
if (-not (Test-Path $workspaceStorage)) {
    Write-Host "  workspaceStorage 目录不存在，跳过。"
} else {
    Get-ChildItem $workspaceStorage -Directory -ErrorAction SilentlyContinue | ForEach-Object {
        $dir = $_.FullName
        $workspaceJson = Join-Path $dir "workspace.json"
        $matched = $false

        if (Test-Path $workspaceJson) {
            $content = Get-Content $workspaceJson -Raw -ErrorAction SilentlyContinue
            if ($content -and ($content -match [regex]::Escape($ProjectName) -or $content -match [regex]::Escape($ProjectPath.Replace('\', '/')))) {
                $matched = $true
            }
        }

        $retrievalDir = Join-Path $dir "anysphere.cursor-retrieval"
        if (-not $matched -and (Test-Path $retrievalDir)) {
            $hits = Select-String -Path (Join-Path $retrievalDir "*") -Pattern $ProjectName -SimpleMatch -ErrorAction SilentlyContinue
            if ($hits) { $matched = $true }
        }

        if ($matched) {
            Remove-IfExists $dir
        }
    }
}

Write-Step "4. 删除 Cursor worktrees"
if (Test-Path $worktreesRoot) {
    Get-ChildItem $worktreesRoot -Directory -ErrorAction SilentlyContinue |
        Where-Object { $_.Name -match [regex]::Escape($ProjectName) } |
        ForEach-Object { Remove-IfExists $_.FullName }
} else {
    Write-Host "  无 worktrees 目录，跳过。"
}

Write-Step "完成"
Write-Host "请重新打开 Cursor。首次打开会重建索引，属正常现象。" -ForegroundColor Green
Write-Host ""
Write-Host "若 Rust 编译产物仍在项目内膨胀，请同时检查系统环境变量 CARGO_TARGET_DIR，"
Write-Host "建议改为 D:\cargo-target\yssbi 或删除该变量。"
