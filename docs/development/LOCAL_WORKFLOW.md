# 本地开发工作流

本项目不依赖 CI 作为开发期验证入口。所有命令都从仓库根目录
`D:\Desktop\YssBI` 运行，并通过 `package.json` 脚本统一调用。

## Cargo 产物目录

根目录 `.cargo/config.toml` 固定 Rust workspace 的产物目录为：

```text
D:\Desktop\YssBI\target
```

不要为日常开发切换到 `src-tauri` 后直接运行 Cargo。请使用下列脚本，
它们都会显式指向 `src-tauri/Cargo.toml`。这避免在仓库根目录和
`src-tauri` 下产生重复的 `target` 目录。

已有的 `src-tauri/target` 是旧构建产物，可以在没有 Cargo 进程运行时
手动删除一次；后续使用本规范不会重新创建它。

## 日常命令

| 目的 | 命令 |
| --- | --- |
| 启动前端开发服务器 | `pnpm dev` |
| 启动 Tauri 桌面应用 | `pnpm tauri:dev` |
| 构建前端 | `pnpm build` |
| 构建桌面安装包 | `pnpm tauri:build` |
| TypeScript 类型检查 | `pnpm typecheck` |
| 前端 Vitest 测试 | `pnpm test` |
| 格式检查 Rust | `pnpm rust:fmt:check` |
| 检查 Rust 编译 | `pnpm rust:check` |
| 测试 Tauri/Rust 主 crate | `pnpm rust:test` |
| 测试科学计算 crate | `pnpm rust:test:sci` |
| 运行宽表统计基准 | `pnpm rust:bench:column-analytics` |
| 运行前端完整验证 | `pnpm verify:frontend` |
| 运行 Rust 完整验证 | `pnpm verify:rust` |
| 运行全部本地验证 | `pnpm verify` |
| 清除标准 Rust 构建产物 | `pnpm rust:clean` |

`pnpm verify` 会执行前端类型检查、前端测试、Rust 格式/编译/测试，以及
`git diff --check`。它不会启动应用、打包安装包或修改项目状态。

## 按改动范围验证

- **React、TypeScript、样式或前端状态改动：**
  `pnpm typecheck`，再运行受影响的 Vitest 测试；提交前运行
  `pnpm verify:frontend`。
- **Rust、Tauri command、项目状态或执行引擎改动：**
  先添加或更新聚焦回归测试，运行 `pnpm rust:check`、受影响的测试，
  提交前运行 `pnpm verify:rust`。
- **`yss-sci` 数值计算改动：**
  运行 `pnpm rust:test:sci`；性能敏感的列统计或分布改动还应运行
  `pnpm rust:bench:column-analytics`，并记录与基线相比的结果。
- **Tauri 打包、权限、插件或构建配置改动：**
  运行 `pnpm tauri:build`，并手动启动应用验证关键路径。

## 清理构建产物

仅在构建缓存损坏、依赖切换或需要释放磁盘空间时运行：

```sh
pnpm rust:clean
```

清理后，下一次 Rust 构建会重新编译依赖，耗时会显著增加。不要在正常
开发流程中把 clean 作为每次验证的一部分。
