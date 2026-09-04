# 本地开发工作流

> Status: Current
> Scope: 开发环境、repository scripts、聚焦测试和按改动范围验证
> Canonical owners: `package.json`、`rust-toolchain.toml` 和相关 manifests 拥有版本/命令事实；本文解释如何使用
> Update when: toolchain source、root scripts、构建入口或验证策略改变时

所有命令从仓库根目录运行。`package.json#scripts` 是本地和自动化任务的唯一命令矩阵；本文不为 TypeScript、Rust 或 Tauri 另建平行手册。设计或实现 feature、fix、refactor 和行为变更前先使用[变更流程](CHANGE_PROCESS.md)。

## 开发环境

- Rust：使用根目录 `rust-toolchain.toml` 固定的版本和 components。
- Node.js：以 `package.json#engines` 为准。
- pnpm：以 `package.json#packageManager` 为准，使用 Corepack/对应版本运行。
- Julia：仅 Julia-backed operations/tests 需要，以 `src-tauri/julia/Project.toml#compat` 为准。

不要在本文复制具体版本号；manifest 是机器可读事实源。

## Cargo output

仓库不提供 `.cargo/` target 覆盖。以 `src-tauri/Cargo.toml` 为 workspace manifest 时，Cargo metadata 当前解析到 `src-tauri/target/`。所有 root Rust scripts 都显式指定该 manifest，并保留 Cargo 默认 build jobs 和 libtest threads。

Windows 上 Rust linking 与 production architecture audit 成本较高。增量循环先运行最窄的 format/check 和受影响测试；不要为每次小编辑重复执行完整 Rust workspace suite。

## Root commands

| 目的                | 聚合命令               | 单栈命令                                                 |
| ------------------- | ---------------------- | -------------------------------------------------------- |
| 安装/同步依赖       | `pnpm install`         | —                                                        |
| 启动 Tauri 桌面应用 | `pnpm dev`             | —                                                        |
| 构建桌面安装包      | `pnpm build`           | —                                                        |
| 类型/编译检查       | `pnpm check`           | `pnpm check:ts`、`pnpm check:rs`                         |
| 静态检查            | `pnpm lint`            | `pnpm lint:ts`、`pnpm lint:rs`                           |
| 测试                | `pnpm test`            | `pnpm test:ts`、`pnpm test:rs`、`pnpm test:architecture` |
| 写入格式化          | `pnpm format`          | `pnpm format:ts`、`pnpm format:rs`                       |
| 只读格式检查        | `pnpm format:check`    | `pnpm format:check:ts`、`pnpm format:check:rs`           |
| 生成 module map     | `pnpm docs:module-map` | check-only：`pnpm docs:module-map:check`                 |
| 完整交付门禁        | `pnpm run ci`          | —                                                        |

`dev` 和 `build` 是完整 Tauri 应用入口。`src-tauri/tauri.conf.json` 的 `beforeDevCommand` / `beforeBuildCommand` 直接运行 Vite，不能回调 root `dev` / `build` scripts，否则会递归。

TypeScript check 使用 `tsc`，lint 使用 Oxlint，format 使用 Oxfmt，tests 使用 Vitest。Rust scripts 使用 Cargo/rustfmt/Clippy 并覆盖 workspace。`pnpm format` 会写入整个仓库；验证时优先使用只读 `pnpm format:check`，不要顺手格式化无关文件。

必须写 `pnpm run ci`：裸 `pnpm ci` 是 pnpm 的 frozen install 命令，不执行同名 package script。`pnpm run ci` 依次运行 format check、TypeScript/Rust checks、Oxlint/Clippy 和完整 TypeScript/Rust tests；它不启动应用或构建安装包。

## Focused tests

单栈 scripts 将其余参数透传给 Vitest 或 Cargo：

```sh
pnpm test:ts <test-file>
pnpm test:ts <test-file> -t "test name"
pnpm test:rs --lib <test-name>
pnpm test:rs --test <integration-target> <test-name>
pnpm test:rs -p <crate-name> <test-name>
pnpm test:architecture
pnpm test:rs --lib architecture_tests
julia --project=src-tauri/julia src-tauri/julia/tests/bayes_fit_tests.jl
```

首次运行 Julia-backed operation/test 前初始化 manifest 环境：

```sh
julia --project=src-tauri/julia -e 'using Pkg; Pkg.instantiate()'
```

## Validation by change scope

- **Documentation only**：运行 `pnpm docs:module-map:check`（若涉及 module map）、documentation contract focused test、`pnpm format:check:ts` 和 `git diff --check`。
- **React、TypeScript、style 或 frontend state**：运行 `pnpm format:check:ts`、`pnpm check:ts`、`pnpm lint:ts` 和受影响的 `pnpm test:ts`。
- **Rust、Tauri command、Project 或 Execution**：运行 `pnpm format:check:rs`、`pnpm check:rs`、`pnpm lint:rs` 和受影响的 `pnpm test:rs`；一个 coherent change 完成后再扩大测试。
- **Architecture policy**：运行 `pnpm test:architecture` 和 `pnpm test:rs --lib architecture_tests`，再执行对应 stack checks。
- **跨前后端、发布或广泛 cross-cutting**：运行一次 `pnpm run ci`。
- **Tauri packaging、permission、plugin 或 build config**：另运行 `pnpm build` 并在目标平台手动验证关键路径。

所有交付都运行 `git diff --check`。只报告有新鲜输出的命令；未运行的相关验证说明原因。

## One-off Cargo maintenance

只有构建缓存损坏、依赖切换或需要释放磁盘空间时才运行：

```sh
cargo clean --manifest-path src-tauri/Cargo.toml
```

这不是日常验证步骤；清理后下次构建会重新编译全部依赖。
