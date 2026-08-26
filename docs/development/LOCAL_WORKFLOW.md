# 本地开发工作流

本项目不依赖 CI 作为开发期验证入口。所有命令都从仓库根目录运行，并通过
`package.json` 脚本统一调用。

## 开发环境

- Node.js `22.22.0` 或更高版本
- pnpm `11.20.0`
- Rust `1.94.0` 或更高版本（Rust 2024 edition）
- Julia `1.10` 或更高版本（仅 Julia-backed operations/tests 需要）

Node.js 最低版本与 React Router 8 的运行时要求保持一致。

应用类型检查和架构审计都使用 TypeScript 7。架构审计通过
`typescript/unstable/sync` 启动 tsgo 项目快照，并使用
`typescript/unstable/ast` 遍历 AST；相关生命周期集中在测试 helper 中管理。

## Cargo 产物目录

根目录 `.cargo/config.toml` 将 Rust workspace 的产物统一放在仓库根目录的
`target/`。

不要为日常开发切换到 `src-tauri` 后直接运行 Cargo。请使用下列脚本，
它们都会显式指向 `src-tauri/Cargo.toml`。这避免在仓库根目录和
`src-tauri` 下产生重复的 `target` 目录。

已有的 `src-tauri/target` 是旧构建产物，可以在没有 Cargo 进程运行时
手动删除一次；后续使用本规范不会重新创建它。

## 日常命令

| 目的 | 命令 |
| --- | --- |
| 安装或同步依赖 | `pnpm install` |
| 启动前端开发服务器 | `pnpm dev` |
| 启动 Tauri 桌面应用 | `pnpm tauri:dev` |
| 构建前端 | `pnpm build` |
| 构建桌面安装包 | `pnpm tauri:build` |
| TypeScript 类型检查 | `pnpm typecheck` |
| 前端 Vitest 测试 | `pnpm test` |
| 测试 Frontend production architecture | `pnpm test:architecture` |
| 格式检查 Rust | `pnpm rust:fmt:check` |
| 检查 Rust 编译 | `pnpm rust:check` |
| 测试 Tauri/Rust library | `pnpm rust:test:lib` |
| 测试 Rust production architecture | `pnpm rust:test:architecture` |
| 测试 Tauri/Rust 主 crate（完整） | `pnpm rust:test` |
| 测试科学计算 crate | `pnpm rust:test:sci` |
| 运行宽表统计基准 | `pnpm rust:bench:column-analytics` |
| 运行前端完整验证 | `pnpm verify:frontend` |
| 运行 Rust 日常静态验证 | `pnpm verify:rust` |
| 运行日常跨栈验证 | `pnpm verify` |
| 运行完整仓库回归 | `pnpm verify:full` |
| 清除标准 Rust 构建产物 | `pnpm rust:clean` |

仓库当前没有 ESLint 或 Prettier script；规范的静态检查入口是
`pnpm typecheck`、`pnpm rust:fmt:check` 和 `pnpm rust:check`。

`pnpm rust:test`、`pnpm rust:test:lib` 和 `pnpm rust:test:sci` 通过 Cargo
`--jobs 1` 序列化 Rust 测试链接，以避免 Windows 链接器内存峰值。
`pnpm rust:check` 和开发构建仍保留 Cargo 的正常并行度。

`pnpm verify:frontend` 精确执行 `pnpm typecheck && pnpm test`。完整 Vitest 已收集
Frontend architecture tests；`pnpm test:architecture` 只作为快速 focused 入口，不在
`verify:frontend` 中重复运行。

`pnpm verify:rust` 精确执行 Rust format check、compile check 与
`pnpm rust:test:architecture`。`pnpm verify` 依次组合 `verify:frontend`、
`verify:rust` 与 `git diff --check`，因此日常跨栈交付会执行两端 architecture gates，
但不会隐式运行完整 Rust runtime、integration 或 SCI suite。Rust 行为改动仍必须先
运行受影响的 focused tests。

`pnpm verify:full` 组合 `verify:frontend`、Rust format/compile checks、完整主 Rust crate、
SCI tests 与 `git diff --check`；它不另行重复调用 focused Rust architecture script，
因为完整主 crate tests 已包含 library architecture tests。仅在发布前、执行引擎/Runtime
跨切面改动、或明确要求完整仓库回归时运行。两种验证命令都不会启动应用、打包安装包
或修改项目状态。

## 聚焦测试

优先通过 `package.json` scripts 运行聚焦测试，使 Cargo 参数和产物目录保持
一致：

```sh
pnpm test:architecture
pnpm test src/path/to/example.test.ts
pnpm test src/path/to/example.test.ts -t "test name"
pnpm rust:test:architecture
pnpm rust:test:lib test_name -- --exact --nocapture
pnpm rust:test --test database_test test_name -- --exact --nocapture
pnpm rust:test:sci test_name -- --exact --nocapture
julia --project=src-tauri/julia src-tauri/julia/tests/bayes_fit_tests.jl
```

### Rust production architecture audit

修改 Rust module、Cargo dependency、re-export、layer classification、command/application seam
或边界债务时，先运行对应的快速 fixture test，再运行真实 production audit：

```sh
pnpm rust:test:architecture
```

真实审计会遍历所有 production targets 并解析 canonical origins，通常比普通 unit test 慢。
它要求实际违规与 `src-tauri/src/architecture_tests/debt/` 的 literal 清单双向完全一致。
新增依赖不能通过增加 broad allow rule 处理；只有目标架构确实允许的 exact capability 才进入
policy，其余项必须绑定维护在 `docs/architecture/` 的边界文档并保留准确 occurrence count。

首次运行 Julia-backed operations/tests 前安装项目环境：

```sh
julia --project=src-tauri/julia -e 'using Pkg; Pkg.instantiate()'
```

## 按改动范围验证

- **React、TypeScript、样式或前端状态改动：**
  `pnpm typecheck`，再运行受影响的 Vitest 测试；提交前运行
  `pnpm verify:frontend`。
- **Rust、Tauri command、项目状态或执行引擎改动：**
  先添加或更新聚焦回归测试，运行 `pnpm rust:check`、受影响的测试，
  涉及 architecture policy 时先运行 `pnpm rust:test:architecture`，提交前运行
  `pnpm verify:rust`。执行引擎跨切面改动或发布前再运行 `pnpm verify:full`。
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
