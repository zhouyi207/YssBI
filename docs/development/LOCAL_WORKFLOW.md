# 本地开发工作流

本项目使用同一组 `package.json` scripts 作为本地和自动化任务入口；托管 CI
不能替代本地验证。所有命令都从仓库根目录运行。新增功能或改变现有行为前，先使用
[功能开发检查清单](FEATURE_PROCESS.md)确认边界、生命周期和验证范围。

## 开发环境

- Node.js `22.22.0` 或更高版本
- pnpm `11.24.0`
- Rust `1.94.0`（由根目录 `rust-toolchain.toml` 固定，Rust 2024 edition）
- Julia `1.10` 或更高版本（仅 Julia-backed operations/tests 需要）

Node.js 最低版本与 React Router 8 的运行时要求保持一致。

应用类型检查和架构审计都使用 TypeScript 7。架构审计通过
`typescript/unstable/sync` 启动 tsgo 项目快照，并使用
`typescript/unstable/ast` 遍历 AST；相关生命周期集中在测试 helper 中管理。


## Cargo 产物目录

仓库不提供 `.cargo/` 覆盖。Cargo 和 Tauri 使用 workspace 默认的
`src-tauri/target/`，所有 Rust scripts 都从仓库根目录运行并显式指向
`src-tauri/Cargo.toml`。

旧的根目录 `target/` 是此前配置产生的构建缓存；确认没有 Cargo 进程使用后可
手动删除。不要同时从不同工作目录运行未带 manifest 的 Cargo 命令，以免再次产生
多套构建产物。

## 命令分组

| 目的 | 聚合命令 | 单栈命令 |
| --- | --- | --- |
| 安装或同步依赖 | `pnpm install` | — |
| 启动 Tauri 桌面应用 | `pnpm dev` | — |
| 构建桌面安装包 | `pnpm build` | — |
| 类型与编译检查 | `pnpm check` | `pnpm check:ts`、`pnpm check:rs` |
| 静态检查 | `pnpm lint` | `pnpm lint:ts`、`pnpm lint:rs` |
| 测试 | `pnpm test` | `pnpm test:ts`、`pnpm test:rs` |
| 写入格式化 | `pnpm format` | `pnpm format:ts`、`pnpm format:rs` |
| 只读格式检查 | `pnpm format:check` | `pnpm format:check:ts`、`pnpm format:check:rs` |
| 完整交付门禁 | `pnpm run ci` | — |

架构与发布收口还提供以下稳定入口：

| 目的 | 命令 |
| --- | --- |
| Frontend production architecture | `pnpm test:architecture` |
| Rust production architecture | `pnpm rust:test:architecture` |
| Frontend 类型检查与完整 Vitest | `pnpm verify:frontend` |
| Rust format/check 与 architecture tests | `pnpm verify:rust` |
| 跨栈 architecture gate | `pnpm verify` |
| Frontend、主 Rust crate 与 yss-sci 完整回归 | `pnpm verify:full` |

`dev` 和 `build` 只表示完整 Tauri 应用入口。`src-tauri/tauri.conf.json` 的
`beforeDevCommand` 和 `beforeBuildCommand` 直接调用 Vite，不能回调这两个
scripts，否则会形成递归。

TypeScript 类型检查由 `tsc` 负责；JavaScript/TypeScript lint 使用 Oxlint，
格式化使用 Oxfmt。Vitest 在继承默认排除规则的基础上忽略 `.worktrees/**`，
主工作区测试和 CI 不扫描隔离 worktree。Rust scripts 使用 `--workspace`/`--all`
覆盖 `yssbi` 和 `yss-sci`，并保持 Cargo 默认构建和链接并行度，不固定 build jobs。

Rust 测试使用 Cargo 内置 test runner。仓库不固定 build jobs 或 test threads，
`pnpm test:rs` 继承 Cargo 与 libtest 的默认并发。

Windows 上 Rust linking 与 production architecture audit 成本较高。增量循环优先运行
`pnpm format:check:rs` 与 `pnpm check:rs`，在一个 coherent Rust change 完成后再运行
受影响的 focused tests；不要为每个小改动重复执行完整 Rust suite。跨 Execution/Database/
SCI 的 release cutover 使用一次 `pnpm verify:full` 批量验证。

`pnpm run ci` 按顺序执行格式检查、TypeScript/Rust 检查、Oxlint/Clippy 和完整
TypeScript/Rust 测试。必须保留 `run`：裸 `pnpm ci` 是 pnpm 的冻结安装命令，
不会执行同名 package script。该门禁不会启动应用或构建安装包；交付前仍需单独
运行 `git diff --check`。Oxfmt 和严格 Clippy 首次接入时会暴露既有基线问题，
不要为了让门禁表面通过而降低规则；应在独立任务中建立格式和 lint 基线。

`pnpm format` 会写入整个仓库，不要把它作为无关改动的顺手操作。验证时优先使用
`pnpm format:check`。

## 聚焦测试

聚焦测试通过单栈 script 向 `cargo test` 透传参数，Cargo 从根目录使用统一
workspace 和 `src-tauri/target/`：

```sh
pnpm test:ts src/path/to/example.test.ts
pnpm test:ts src/path/to/example.test.ts -t "test name"
pnpm test:rs --lib completed_task_has_terminal_status
pnpm test:rs --test database_test test_duckdb_query_page_and_schema_without_full_load
pnpm test:rs -p yss-sci test_name
pnpm test:architecture
pnpm rust:test:architecture
julia --project=src-tauri/julia src-tauri/julia/tests/bayes_fit_tests.jl
```

`pnpm verify:frontend` 精确执行 TypeScript check 与完整 Vitest；完整 Vitest 已包含
Frontend architecture tests。`pnpm verify:rust` 运行 Rust format/check 与 architecture
tests。`pnpm verify:full` 运行 Frontend、主 Rust crate、yss-sci 与最终 `git diff --check`，
不会启动应用或构建安装包。

首次运行 Julia-backed operations/tests 前安装项目环境：

```sh
julia --project=src-tauri/julia -e 'using Pkg; Pkg.instantiate()'
```

## 按改动范围验证

- **React、TypeScript、样式或前端状态改动：**
  运行 `pnpm format:check:ts`、`pnpm check:ts`、`pnpm lint:ts` 和受影响的
  `pnpm test:ts` 测试。
- **Rust、Tauri command、项目状态或执行引擎改动：**
  先添加或更新聚焦回归测试，运行 `pnpm format:check:rs`、`pnpm check:rs`、
  `pnpm lint:rs` 和受影响的 `pnpm test:rs` 测试。
- **跨前后端、发布或执行引擎跨切面改动：**
  运行 `pnpm run ci`；Tauri 打包、权限、插件或构建配置改动还需运行 `pnpm build`
  并手动验证关键路径。
- **`yss-sci` 性能敏感改动：**
  从仓库根目录运行
  `cargo bench --manifest-path src-tauri/Cargo.toml -p yss-sci --bench column_analytics_wide`
  并记录与基线相比的结果。一次性专用命令不再占用 `package.json` scripts。

## 一次性 Cargo 维护命令

只在构建缓存损坏、依赖切换或需要释放磁盘空间时，从仓库根目录运行：

```sh
cargo clean --manifest-path src-tauri/Cargo.toml
```

所有一次性 Cargo 命令都必须显式使用 `src-tauri/Cargo.toml`。清理后下一次
Rust 构建会重新编译依赖，不要把 clean 纳入日常验证。
