# Cursor 缓存清理（Windows）

当 Cursor 卡顿、AI 回复变慢时，可能是全局数据库或 workspace 索引膨胀。以下命令等价于 macOS 版清理流程。

## 前置条件

**必须先完全退出 Cursor**（所有窗口关闭）：

- 菜单：**File → Exit**（或 **Cursor → Quit Cursor**）
- 确认任务管理器中没有 `Cursor.exe` 进程

---

## 方式一：一键脚本（推荐）

```powershell
Set-ExecutionPolicy -Scope Process -ExecutionPolicy Bypass
& "D:\Desktop\YssBI\docs\cursor-cleanup-windows.ps1"
```

自动结束 Cursor 并清理：

```powershell
& "D:\Desktop\YssBI\docs\cursor-cleanup-windows.ps1" -ForceStopCursor
```

---

## 方式二：手动命令

在 **PowerShell** 中逐条执行（路径已按 Windows 适配）：

### 1. 确认 Cursor 已退出

```powershell
Get-Process -Name "Cursor*" -ErrorAction SilentlyContinue
# 若有输出，请先退出 Cursor，或执行：
# Get-Process -Name "Cursor*" | Stop-Process -Force
```

### 2. 删除 globalStorage

强制重建全局存储数据库（可清除混乱的 embedding / workspace 关联）：

```powershell
$globalStorage = "$env:APPDATA\Cursor\User\globalStorage"

Remove-Item "$globalStorage\storage.json" -Force -ErrorAction SilentlyContinue
Remove-Item "$globalStorage\state.vscdb*" -Force -ErrorAction SilentlyContinue
```

对应 macOS 路径：

| macOS | Windows |
|-------|---------|
| `$HOME/Library/Application Support/Cursor/User/globalStorage/` | `%APPDATA%\Cursor\User\globalStorage\` |

### 3. 删除 YssBI 相关的 workspaceStorage

查找引用：

```powershell
$workspaceStorage = "$env:APPDATA\Cursor\User\workspaceStorage"
Select-String -Path "$workspaceStorage\*\workspace.json" -Pattern "YssBI" -SimpleMatch
```

删除命中的目录（示例 hash，以实际 grep 结果为准）：

```powershell
Remove-Item "$workspaceStorage\160c786794bfb2753c9961d864f5e19f" -Recurse -Force
Remove-Item "$workspaceStorage\52c4eeac622d4fb0f8a34a6f32f9d034" -Recurse -Force
```

### 4. 删除 worktrees

```powershell
$worktrees = "$env:USERPROFILE\.cursor\worktrees"
Remove-Item "$worktrees\YssBI" -Recurse -Force -ErrorAction SilentlyContinue
Remove-Item "$worktrees\yssbi" -Recurse -Force -ErrorAction SilentlyContinue
```

对应 macOS：`~/.cursor/worktrees/project-name`

---

## 清理后

1. 重新打开 Cursor 与本项目
2. 首次索引会稍慢，之后应恢复正常
3. 若 `state.vscdb` 再次超过数 GB，可定期重复上述步骤

## 相关：Rust 构建缓存

若项目目录下 `target/` 再次膨胀，请检查系统环境变量：

- **变量名**：`CARGO_TARGET_DIR`
- **建议值**：`D:\cargo-target\yssbi`（放在项目外）
- **不建议**：`D:\Desktop\YssBI\target`（会导致 Cursor 持续索引 GB 级文件）

项目内已配置 `.cursorignore` 与 `.vscode/settings.json` 以减轻索引压力。
