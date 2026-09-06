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

Windows 上 Rust linking 与 production architecture audit 成本较高。增量循环先选择 package、target 和相关用例；小修改不默认启动整个 workspace 的 check、lint 或 test。完整验证的升级条件见下文 L1/L2/L3。

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

TypeScript check 使用 `tsc`，lint 使用 Oxlint，format 使用 Oxfmt，tests 使用 Vitest。Rust 的 `check:rs`、`lint:rs`、`test:rs` 是 workspace 级入口，`lint:rs` 还包含全部 targets/features；不用于小修改的固定收尾。按 crate 的入口如下，调用时必须显式传入 `-p`：

| 目的     | 按 crate 入口                           |
| -------- | --------------------------------------- |
| 编译检查 | `pnpm check:rs:package -p <crate-name>` |
| 静态检查 | `pnpm lint:rs:package -p <crate-name>`  |
| 测试     | `pnpm test:rs:package -p <crate-name>`  |

这些入口不自动添加 `--workspace`、`--all-targets` 或 `--all-features`；根据改动选择 target 和必要 features，依赖仍可能需要编译。保留现有全量命令和 `ci` 的完整语义，不把它们改成局部验证。

`pnpm format` 会写入整个仓库，不要顺手格式化无关文件。局部文档/前端修改使用 `pnpm format:check:ts <changed-files>`；`pnpm format:check:rs` 仍是只读的 workspace 格式检查，不编译、链接或执行测试。

必须写 `pnpm run ci`：裸 `pnpm ci` 是 pnpm 的 frozen install 命令，不执行同名 package script。`pnpm run ci` 依次运行 format check、TypeScript/Rust checks、Oxlint/Clippy 和完整 TypeScript/Rust tests；它不启动应用或构建安装包。

## Focused validation

单栈 scripts 将其余参数透传给 Vitest 或 Cargo。以下是模板，替换文件、crate、target 和测试名称后执行：

```sh
pnpm test:ts <test-file>
pnpm test:ts <test-file> -t "test name"
pnpm check:rs:package -p <crate-name> --tests
pnpm lint:rs:package -p <crate-name> --all-targets '--' -D warnings
pnpm test:rs:package -p <crate-name> --lib <test-name>
pnpm test:rs:package -p <crate-name> --test <integration-target> <test-name>
```

Clippy 示例将参数分隔符 `'--'` 写成字符串，避免 PowerShell 调用 `pnpm.ps1` 时消耗分隔符，确保 `-D warnings` 传给 Clippy。

Rust 的 `-p` 选择 package，`--lib` / `--test` 选择测试目标，测试名称过滤目标内运行的用例。只过滤名称不会只编译那个测试函数。`--tests`、`--all-targets` 也应按需要选择，迭代时可以先用 `--lib`；不要默认追加 `--all-features`。

`test:rs` 固定带 `--workspace`，即使追加 `-p` 仍选择整个 workspace；`check:rs` 和 `lint:rs` 也不是按 crate 入口。需要聚焦时使用对应的 `:package` 命令，不通过全局限制 build jobs/test threads 替代正确的范围选择。

确认输出中实际执行了相关测试；零匹配不能算修复通过。`cargo check` 不执行最终代码生成或链接，`--no-run` 只构建测试程序，二者都不能替代运行结果。

仅在相关改动需要时选择专项检查，例如文档契约、Rust 根包架构门禁或 Julia-backed 测试：

```sh
pnpm test:ts src/tests/architecture/documentationContract.test.ts
pnpm test:rs:package -p yssbi --lib architecture_tests
julia --project=src-tauri/julia src-tauri/julia/tests/bayes_fit_tests.jl
```

Rust 架构门禁仍需构建根包测试目标，不保证很快。`pnpm test:architecture` 是完整前端架构检查，按涉及的依赖边界、分类、策略或 source discovery 决定是否运行；不要为普通局部实现修改机械执行完整架构审计。

Graph diagnostic 词汇或模板变更后运行 `pnpm generate:diagnostics`，只读校验为 `pnpm generate:diagnostics:check`。生成表由 Rust definitions 拥有。

i18n 词条、翻译文案、语言切换和回退不单独维护测试或库存夹具。业务测试可以使用翻译 mock，但断言应保护状态、交互、契约或敏感信息边界，不枚举语言和词条。

首次运行 Julia-backed operation/test 前初始化 manifest 环境：

```sh
julia --project=src-tauri/julia -e 'using Pkg; Pkg.instantiate()'
```

## Validation by change scope

| 层级               | 使用时机                                                        | 验证范围                                                                     |
| ------------------ | --------------------------------------------------------------- | ---------------------------------------------------------------------------- |
| L1：聚焦验证       | 修复或实现过程中的迭代                                          | 相关前端测试文件，或指定 Rust package、target、回归用例；按需选择局部 check  |
| L2：受影响范围验证 | 一个连贯改动准备交付                                            | 修改模块和受影响调用方的测试、相关契约检查、对应范围的 lint/check 和格式检查 |
| L3：完整门禁       | 用户明确要求、既定合并/发布门禁，或影响无法可靠限定的高风险改动 | 一次 `pnpm run ci`，再按风险补充专门验证                                     |

执行前说明受影响模块、所选检查和理由；升级到 L3 前说明具体触发原因。跨前后端本身不触发 L3，也不把每个小任务当作一次全仓交付。选择层级依据行为和依赖的影响范围，不依据修改行数：

- **文档和命令入口**：检查变更文件格式、相关命令配置和 documentation contract。只有 module map 受影响且已有检查未覆盖时，才另外运行 `pnpm docs:module-map:check`。仅调整验证策略不触发全仓架构审计。
- **前端局部行为**：选择相关测试文件；L2 补必要的 `pnpm check:ts`、`pnpm lint:ts` 和局部格式检查。`lint:ts` 固定扫描整个前端，追加文件不能缩小范围；按需运行，不因前端修改自动运行 Rust 检查。
- **Rust 局部实现**：使用对应 `:package` 入口，显式选择 crate 和 target；L2 验证相关模块及调用方，不机械运行 workspace 级 check/Clippy。
- **公共 API、共享类型、序列化和跨模块行为**：先评估直接和间接受影响的消费者，补充相应契约检查。比如 Graph 类型语义可能影响 Compiler、Execution 和前端 Projection，不能只以 Graph Analysis 自身测试通过收尾；只有影响无法可靠限定时才升级 L3。
- **架构策略、依赖边界或 source discovery**：运行对应的门禁回归和真实审计；涉及全局策略时扩大到对应语言的完整架构门禁，不自动扩大到所有业务测试。
- **toolchain、workspace 公共依赖、features 或测试基础设施**：评估受影响构建图；广泛影响无法可靠限定时升级 L3。仅增加明确的聚焦命令入口不等于改变整个构建图。
- **Tauri packaging、permission、plugin 或 build config**：按影响补充 `pnpm build` 及目标平台手动验证；局部业务代码修改不默认重新构建安装包。

不要每改一次代码就机械执行 format → check → Clippy → test。L1 先选能验证当前问题的检查，L2 再补遗漏的相关验证。同一次任务中，相关代码、依赖、配置和测试输入未变且已有真实结果的检查不重复运行；出现新修改、失败或未解决风险时才重跑或扩大。测试失败先诊断，不用全量重跑代替定位。

所有交付都运行 `git diff --check`，并报告实际命令、范围、结果及未运行/未完成的相关验证。聚焦测试通过不等于完整门禁通过；冷编译、环境初始化失败或测试进程尚未结束也不能标成通过。

## Validation cost

验证异常耗时时先报告所处阶段并重新评估范围，不继续叠加更多检查。区分编译/链接与用例执行；必要时对一个代表性 crate 做一次构建耗时检查：

```sh
pnpm test:rs:package -p <crate-name> --lib --no-run --timings
```

这会构建但不运行测试，并生成构建耗时报告，不是每次修复的固定步骤。优先收窄 package/target、保留可复用构建产物，再检查不必要的依赖/features。只有测量发现慢用例或重复覆盖时，才优化初始化、数据规模或合并重复测试；不要为提速随意减少覆盖、跳过测试或拆分 crate。

如后续引入自动受影响测试选择，必须为共享契约、生成文件和源码扫描式架构测试设置显式触发规则；仅依赖静态 import 关系的选择不能覆盖这些风险。

## One-off Cargo maintenance

只有构建缓存损坏、依赖切换或需要释放磁盘空间时才运行：

```sh
cargo clean --manifest-path src-tauri/Cargo.toml
```

这不是日常验证步骤；清理后下次构建会重新编译全部依赖。
