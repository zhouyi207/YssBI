# 在正式打包分发前，可以等渲染完毕再显示窗口
# 在目前的开发环境中，不要这样做，可以取消 debug
# 项目未发布 不做任何迁移处理

每次更新版本都需要

由于历史代码重构原因，目前项目中存在许多的历史遗留代码，或多余或逻辑重复或实现低效；请检查整体项目，寻找出项目中的重复逻辑和未使用的逻辑，分析必要性，如果有更高效的更干净的架构请添加到 todo 的 v1.0 待办中，如果单纯的逻辑重复或者多余，也请添加到 v1.0 待办中

请分析这个问题有没有必要修复，如果有必要，则使用高效且干净的架构来执行这个逻辑，同时清除掉无效逻辑代码和重复逻辑代码

重复逻辑问题？无效逻辑问题？代码漂移问题？多事实源问题？代码冲突问题？无效函数问题？deprecated 兼容问题？

LOCAL_WORKFLOW.md 的很多文件内容需要处理

https://rig.rs/ 这个页面可以作为我的 yss 页面的参考

[@improve-codebase-architecture](zed:///agent/skill?name=improve-codebase-architecture&source=global&path=C%3A%5CUsers%5Czhou.yi31%5C.agents%5Cskills%5Cimprove-codebase-architecture%5CSKILL.md) [@grill-me](zed:///agent/skill?name=grill-me&source=global&path=C%3A%5CUsers%5Czhou.yi31%5C.agents%5Cskills%5Cgrill-me%5CSKILL.md) [@vercel-react-best-practices](zed:///agent/skill?name=vercel-react-best-practices&source=global&path=C%3A%5CUsers%5Czhou.yi31%5C.agents%5Cskills%5Cvercel-react-best-practices%5CSKILL.md) [@vercel-composition-patterns](zed:///agent/skill?name=vercel-composition-patterns&source=global&path=C%3A%5CUsers%5Czhou.yi31%5C.agents%5Cskills%5Cvercel-composition-patterns%5CSKILL.md) 请检查前端 react 架构，是否有 重复逻辑问题？无效逻辑问题？代码漂移问题？多事实源问题？代码冲突问题？无效函数问题？deprecated 兼容问题？如果有请修复，并优化代码架构，同时删除边缘测试，在这里有很多的测试是低效的完全没必要的

[@improve-codebase-architecture](zed:///agent/skill?name=improve-codebase-architecture&source=global&path=C%3A%5CUsers%5Czhou.yi31%5C.agents%5Cskills%5Cimprove-codebase-architecture%5CSKILL.md) [@grill-me](zed:///agent/skill?name=grill-me&source=global&path=C%3A%5CUsers%5Czhou.yi31%5C.agents%5Cskills%5Cgrill-me%5CSKILL.md) 请先检查 rust 后端架构，是否有重复逻辑问题？无效逻辑问题？代码漂移问题？多事实源问题？代码冲突问题？无效函数问题？deprecated 兼容问题？如果有请修复，并优化代码架构，同时删除边缘测试，在这里有很多的测试是低效的完全没必要的

测试应该保护未来仍然成立的行为或架构约束，而不是永久证明某次历史重构确实做过。

```
同步改四处版本（例如 0.1.1）：

src-tauri/Cargo.toml
src-tauri/tauri.conf.json
package.json
src/app/appConfig/appLinks.ts


提交并 push，或手动再跑 publish.yml
```

codex resume 01a03e97-3969-7c82-85e9-8d2071050906 --yolo


已完成汇总文档：

  docs/superpowers/plans/2026-08-27-architecture-decoupling-progress-and-next-steps.md

  文档包含：

  - 已完成、已集成、进行中、待实现的完整状态矩阵。
  - Strict Frontend/Rust、Backend Tasks 1–5a、Project–Graph Task 1 的实现与提交记录。
  - 当前 architecture-final、各 worktree 和验证状态。
  - 当前进行中的 Backend Task 5b Tabular/Polars 拆分。
  - Backend、Project–Graph、Execution、Presentation、Frontend Application 的后续依赖顺序。
  - 最终原子切换、债务清零和验证门槛。
  - 明确禁止兼容层、双路线、动态 debt baseline。

  同时已向 TODO.md 的 2026.08.27 标题追加两条摘要，未改写既有内容。

  最新集成分支 d042b4f9 已重新确认：

  - pnpm rust:fmt:check：通过
  - pnpm rust:check：通过
  - git diff --check：通过

  docs/superpowers 按仓库规则保持 git-ignored、未提交；主工作区中的用户代码改动未被触碰

# DOLIST


## 2026.08.28

- [ ] 将 `package.json` 收敛为 14 个稳定任务入口，删除 preview、原始 Tauri、library test 及分层 verify convenience scripts。
- [ ] 移除 Rust 测试命令硬编码的 `--jobs 1`，恢复 Cargo 默认构建/链接并行度，并保留低内存机器的按需降并行说明。
- [ ] 保留 `RUST_TEST_THREADS=1` 作为测试运行期数据库与全局状态隔离，不再与 Cargo 构建并行度混为一谈。
- [ ] 使用 `rust-toolchain.toml` 固定 Rust 1.94.0，并让 GitHub Release workflow 安装同一版本。
- [ ] 新增 YssBI 功能开发检查清单，覆盖 authority、IPC、生命周期、输出、Dockview、性能、测试及分发决策。
- [ ] 同步本地工作流、仓库规则、README 与图标生成命令，确保文档只引用保留的任务入口。
- [ ] 将任务入口重组为 dev/build 与 check、lint、test、format、format:check 五组，并新增统一 `pnpm ci` 聚合门禁。
- [ ] 将前端 lint/format 工具收敛为 Oxlint 与 Oxfmt，避免 TypeScript 7 与 typescript-eslint peer 范围不兼容。
- [ ] 将 Rust 检查、Clippy、测试和格式命令统一覆盖 workspace，同时继续使用 Cargo 默认构建并行度。
- [ ] 调整 Tauri 的 Vite 前置命令为直接执行，避免 `pnpm dev`/`pnpm build` 改为 Tauri 入口后递归调用。
- [ ] 移除 benchmark/clean convenience scripts，并在本地工作流中保留显式 manifest 的一次性 Cargo 命令。
- [ ] 明确同名聚合脚本必须通过 `pnpm run ci` 调用，避免裸 `pnpm ci` 误执行 pnpm 的冻结安装命令。
- [ ] 将 Rust workspace 测试入口从 `cargo test` 切换为 cargo-nextest 0.9.143，并保留现有参数透传能力。
- [ ] 使用 Nextest 默认 CPU 并发运行测试，不增加仓库级 `test-threads` 限制。
- [ ] 删除根目录 `.cargo/` 及其 target/env 覆盖，让 Cargo 与 Tauri 恢复标准 `src-tauri/target/`。
- [ ] 更新开发工作流和仓库规则，记录 Nextest 安装方式及单次低并行调试参数。
- [ ] 将 Rust 聚焦测试示例改用 Nextest 原生 filterset，避免 libtest 参数分隔符和 `--nocapture` 强制串行。
- [ ] 以最终确认的 18 个 scripts 为准：保留 dev/build、五组聚合/TS/Rust 任务和 ci；早期“14 个入口”记录不再代表最终矩阵。
- [ ] 以 Nextest 默认 CPU 并发为最终方案，删除 `RUST_TEST_THREADS=1` 环境覆盖；早期保留串行限制的记录已被后续确认取代。
- [ ] 修正 `database_test` 对已迁移 `bind_duckdb_instance` 的旧 Application 导入，改用 Database 模块现有公开 seam。
- [ ] 使用 cargo-nextest 验证 DuckDB 绑定聚焦用例及完整 `database_test` integration binary，确认 19 个测试全部通过。
- [ ] 以最终确认方案取消 cargo-nextest，恢复 `cargo test --workspace`，并继续使用 Cargo/libtest 默认并发而不设置仓库级限制。
- [ ] 删除维护文档中的 Nextest 安装、filterset 和降并发说明，并卸载本次通过 WinGet 安装的全局 cargo-nextest。
- [ ] 使用原生 Cargo 重新验证 DuckDB 绑定聚焦用例及完整 `database_test`，确认 19 个 integration tests 全部通过。
- [ ] 在 Vitest 配置中继承默认 exclude 并统一排除 `.worktrees/**`，避免主工作区测试重复扫描隔离 worktree。
- [ ] 通过测试发现 RED/GREEN 验证 worktree 测试从大量命中降为 0；完整前端套件仅保留主工作区既有的 Dockview 默认值失败。
- [ ] 按最终决策移除 Vite 的 `**/target/**` watch 忽略项，仅保留 `**/src-tauri/**` 以覆盖标准 Cargo 产物目录。
- [ ] 将任务脚本、Vitest 排除、Vite watch、Database integration 与开发文档拆分为独立 Git 提交，并在推送前逐层复核 staged diff。
- [ ] 完成 Backend Task 5b 的 Pure Tabular ordered contract、manual serde 与 duplicate/ragged shape 校验，保持既有 wire shape。
- [ ] 将变量 JSON/handle normalization、Polars materialization、DataFrame I/O 分别归入 Project、Backend adapter、Database owner，并删除旧 mixed tabular owners。
- [ ] 补充 typed tabular/materialization/I/O/DTO mapping errors、atomic normalization 与 architecture/debt guard，避免 raw backend prose 和 lossy unsigned conversion。
- [ ] 通过 tabular 聚焦回归、数据库编辑 integration 回归、Rust 编译/格式/debt 验证及独立 review；隔离 worktree 已 review-clean，等待集成授权。
- [ ] 将本地 `shadcn` 提交变基到最新 `origin/shadcn`，吸收远端 10 个提交并保留本地测试提交。
- [ ] 合并自动 stash 的 `TODO.md` 日期段冲突，保留远端记录与本地 Backend Task 5b 摘要。
- [ ] 分析 graph canvas 节点右键“选择节点…”链路，确认它只在既有画布节点之间切换选择，而右键目标节点已在菜单打开前被选中。
- [ ] 核对空白画布 `NodePalette` 的键盘行为，确认搜索框会自动聚焦，但当前未处理上下方向键与 Enter，创建仍只由节点项点击触发。
- [ ] 明确后续修正方向：移除临时节点选择器，将可见叶节点的键盘高亮与 Enter 创建接入空白处节点树，并复用现有 descriptor 创建链路。
- [ ] 为 blank canvas `NodePalette` 增加首个可见叶节点初始化、上下方向键切换、Enter descriptor 创建、活动项高亮滚动及屏幕阅读器状态播报。
- [ ] 在搜索、分类展开状态和 catalog 投影变化时重置活动节点，并在 IME 文本组合期间忽略 palette 快捷键，避免旧选择复活或误创建。
- [ ] 删除节点右键菜单的临时“选择节点…”入口、Canvas picker 状态、overlay model、projection 缓存、专用组件测试及中英文废弃文案。
- [ ] 增加 NodePalette 键盘创建、IME 防误触和筛选重置回归覆盖，完成 34 个相关测试、TypeScript 检查、Oxlint 与独立代码审查。
- [ ] 记录仓库级 Oxfmt 当前因缺少配置并命中 1311 个既有文件而失败，本任务不批量格式化或改写无关基线文件。
- [ ] 审计前端 5 个 architecture-contract 测试，确认其仅占前端测试约 2.69%，但约 1654 行源码审计设施和约 22.89 秒聚焦运行成本明显失衡。
- [ ] 将 `nodeIdentityArchitectureContract.test.ts` 与 observability omnibus source scan 列为优先清理对象，改由行为测试、类型系统、resolved dependency lint 和配置审计分别承担契约。
- [ ] 审计 Rust commands 目录 45 个测试，确认 43 个属于 wire、错误、事件或命令行为覆盖，不能因 `command_*_tests` 命名整体删除。
- [ ] 标记两个读取 Rust 实现源码并匹配函数名或字符串的结构测试，后续应改为可观测事件断言、seam instrumentation、benchmark 或直接删除。
- [ ] 确认现有 OLS/WLS golden 覆盖数值面较广但缺少外部工具版本、命令、数据哈希、生成脚本和原始参考输出，当前更接近内部行为快照。
- [ ] 将测试治理从硬性数量比例改为价值准入，优先建设可复现的 Stata/R/statsmodels 参考 fixture，并按每个独立高风险统计 seam 保留一个代表案例。
- [ ] 完成 P0 测试清理：删除两个 Rust 源码文本 oracle，移除 901 行 node identity AST analyzer，将 observability omnibus scan 收敛为两条仓库级政策并把 DTO/status 断言迁移到 owner wire tests。


## 2026.08.27

- [ ] 完成 Rig、rmcp、assistant-ui 与 YssBI 当前/目标模块边界的架构审计，确认内部 Agent 与外部 MCP 复用 Application 能力而不共享协议实现。
- [ ] 将长期能力入口收敛为 Application-owned capability gateway，区分领域请求、model-facing capability schema、Tauri DTO 与 MCP/Rig adapter mapping。
- [ ] 规划 typed `thiserror` owner errors、稳定安全 failure code/details、incident correlation 与 Rig/MCP/Tauri 各自的穷举错误映射。
- [ ] 规划 actor/session-bound policy、一次性 approval grant、invocation idempotency、deadline/cancellation 和非诊断型 invocation ledger。
- [ ] 规划 revision-aware `apply_graph_edit` 批处理与可验证 undo receipts，明确现有 `OperationId` 只作 correlation、不得充当 AI transaction 或幂等键。
- [ ] 规划 Rust-authoritative assistant session/event stream、assistant-ui ExternalStore projection，以及 loopback MCP transport、认证和 project-session binding。
- [ ] 明确 Assistant Host 只依赖其自有 `AgentDriverPort`，Rig adapter 实现该 port 并由 composition root 注入，禁止 Application 反向 import Rig/rmcp。
- [ ] 明确 Rig 与 MCP adapter 只能依赖 Capability Gateway public interface，禁止直接访问 Project、Graph、Execution authority 或互相调用。
- [ ] 为 Automation contracts、Application、Rig、MCP、Tauri schema/event 建立单向依赖与 exact import architecture guards，防止 service locator、barrel re-export 和双向回调形成隐式环。
- [ ] 新增 Automation Capability Foundation 书面规范，固定 Foundation 与 Internal Assistant、Assistant Frontend、MCP Server、MCP Client 四阶段拆分及前置迁移顺序。
- [ ] 将 `automation_contract` 固定为 Pure Leaf、将 Rig/MCP 固定为第十六层 Automation Adapter，并定义 composition root 唯一 concrete constructor ownership。
- [ ] 固定 `inspect_graph`、`search_node_catalog`、`inspect_dataset_schema`、`apply_graph_edit` 四个 closed typed capability contract 与有界数据暴露规则。
- [ ] 将 `apply_graph_edit` 设计为 clientKey-aware 单 graph 批处理，要求一次 staged validation、一次 Project commit、一次 revision/history/publication。
- [ ] 固定 principal/session-bound approval、binding nonce invocation ID、无淘汰 session ledger、typed failure/incident 和 commit point-of-no-return 语义。
- [ ] 定义 Capability Gateway 五类 public interface、architecture fitness functions、代表性回归测试与 production-unrouted Foundation 验收条件。
- [ ] 批准 Automation Capability Foundation 规范并将状态更新为已批准，保持 Foundation 与后续 Internal Assistant/Frontend/MCP 设计分离。
- [ ] 新增 Capability Foundation 十一任务实施计划，设置最终 ApplicationSessionSlot、Project–Graph、Database 与 strict policy 的硬前置 gate。
- [ ] 规划第十六层 Automation Adapter、Pure Leaf capability contracts、schemars 1.2.2 exact allowlist 与 contract/schema golden 的 RED–GREEN 任务。
- [ ] 规划 loaded graph inspection、single-route catalog search、no-row dataset schema inspection 三个独立 Application 深 module 及 currentness 回归。
- [ ] 规划 clientKey graph edit batch 通过原 mutation owner 构造 sealed candidate，保留唯一 `mutate_graph_in_session`/Project commit caller。
- [ ] 规划 Gateway binding、policy、approval、no-eviction ledger、OperationId admission、deadline/cancellation 与 commit point-of-no-return 的完整状态测试。
- [ ] 在实施计划中加入每任务 TODO、focused tests、architecture tests、baseline hash 对比、untracked whitespace 与 production-unrouted 验证门禁。
- [ ] 复核所谓 2026.08.26 plans 实为 `2026-08-25-*` 文件并于 8 月 26 日完成最后修改，确认其 exact cross-plan 顺序仍是 Foundation 的权威前置。
- [ ] 重新审计 Foundation 的 15 个硬前置最终 seam，当前工作树存在 0 个、缺失 15 个，因此禁止立即执行 Foundation Task 1。
- [ ] 确认 Foundation 与 Strict、Backend、Project–Graph、Execution、Presentation 在 architecture policy、ApplicationState、Graph mutation、Database session、Project history 等文件有预期后置重叠，并行执行会形成真实冲突。
- [ ] 固定安全插入点为 Presentation Tasks 4–8 完成之后；Foundation 可在 Frontend Application Boundaries 前后执行，但推荐同一工作树顺序执行以避免 TODO/文档协调冲突。
- [ ] 明确当前可执行起点是 Strict Architecture Policy，而不是为当前 ProjectState/node_system 增加 Capability compatibility bridge。
- [ ] 新增 Assistant UI Workbench Shell 书面规范，将本切片限定为 frontend-only、无模型、无 IPC、发送禁用的可见 Workbench 壳层。
- [ ] 固定 Details 为 canonical right edge 的唯一 permanent view，禁止 panel、tab 及包含 Details 的整个 group 绕过 move/split/remove policy。
- [ ] 将 Assistant 定义为默认与 Details 同组但可独立移动、split、关闭并由 View 菜单重建的 layout-persisted singleton。
- [ ] 规划 assistant-ui ExternalStoreRuntime 空投影与 Application-owned adapter，禁止 LocalRuntime、Zustand message authority、AI SDK、MCP 和临时 backend 路径。
- [ ] 明确 Assistant reveal/restore/reset/project replacement 语义，并同步 Strict、Frontend plans 与维护架构文档的后续修改范围。
- [ ] 记录本切片不新增或运行测试，仅以 typecheck、build、diff 和只读 source audit 作为实施验证门禁。
- [ ] 批准 Assistant UI Workbench Shell 书面规范，并将规范状态锁定为 2026.08.27 已批准。
- [ ] 新增 Assistant UI Workbench Shell 七任务实施计划，覆盖依赖、runtime adapter、View、Dockview policy、layout、菜单、文档同步与无测试交付门禁。
- [ ] 以 assistant-ui 0.15.16 官方 ExternalStoreRuntime、Composer 与 Thread primitives 契约复核计划中的 runtime/provider/send gate 接口。
- [ ] 自审实施计划的规范覆盖、类型命名、代码围栏、占位符和测试命令，保持当前切片不运行测试且不执行 Git staging/commit。
- [ ] 安装唯一直接依赖 `@assistant-ui/react`，不引入 AI SDK、Cloud、MCP、Lucide 或 assistant-ui CLI 生成源码。
- [ ] 新增 Application-owned Assistant ExternalStoreRuntime 壳层，使用不可变空消息、固定非运行态与严格发送禁用。
- [ ] 新增窄 AssistantRuntimeProvider，不建立 Zustand、React message store、Service、Tauri 或后端执行路径。
- [ ] 新增 AssistantPanel 与紧凑 AssistantThread，使用 assistant-ui headless primitives 组合项目现有 shadcn 组件。
- [ ] 使用项目 ScrollArea、Empty、Button 与 react-icons/vsc 构建空态和可编辑但不可发送的 composer。
- [ ] 增加 Assistant 壳层中英文文案与可访问标签，不引入 backend prose、toast、浏览器对话框或第二 UI 库。
- [ ] 将 `view:assistant` 与 `Assistant` component 纳入 Workbench closed metadata，并保持 Details 为唯一 persistent view。
- [ ] 为 Assistant 设置 right-edge deterministic home，但不建立固定 placement、Activity membership 或布局镜像。
- [ ] 增加包含 Details 的整个 Dockview group 拖动保护，同时保留 Assistant 独立 tab 的普通移动与 split 能力。
- [ ] 在新默认布局中固定 Details index 0、Assistant index 1，并保持 Project 为启动 active panel。
- [ ] 实现 Assistant existing reveal 保留实际位置、missing recreate 原子回到 Details 后，以及显式 Reset 回归默认 home。
- [ ] 保持 restore 缺失即关闭、project replacement 保留 Assistant、固定 Details group 拒绝批量 Close Group 的既有语义。
- [ ] 将 Assistant tab 图标统一为既有 `react-icons/vsc` 的 `VscSparkle`，并保持普通 tab 的关闭/中键/右键行为。
- [ ] 在 View 菜单增加由 root Dockview live projection 驱动的 Assistant checkbox 与 toggle action，不新增 visibility store。
- [ ] 完成 Task 5 的 Dockview panel/tab/menu 注册并通过允许的 TypeScript 检查，未新增或运行测试。
- [ ] 更新维护架构文档，纠正 Details lazy/session-only 旧描述并记录 Assistant、Diagnostics、restore/reset/project replacement 当前契约。
- [ ] 将 `@assistant-ui/react` 纳入待执行 Strict Architecture 的 33 项 exact declaration 与 Application/View 精确 consumer policy。
- [ ] 将 Assistant Application adapter、View composition、无前端 message authority 与 Dockview cutover 纳入 Frontend Application Boundaries 规范和计划。
- [ ] 完成 Assistant UI Workbench Shell 无测试交付门禁，以新鲜 typecheck、production build、source audit 与 diff check 验证本切片。
- [ ] 确认 assistant-ui 仅由批准的 Application/View 文件直接引用，未产生 LocalRuntime、Zustand conversation authority、Tauri 或临时 backend 路径。
- [ ] 确认 root Dockview/Logs nested constructor 数量不变，Details 仍是唯一 fixed view，Assistant 保持普通可移动持久化 panel。
- [ ] 在 `docs/superpowers/plans` 新增架构解耦进度总览，按 review-clean、已集成、进行中和待实现区分 strict policy、Backend、Project–Graph、Execution、Presentation 与 Frontend Application 工作。
- [ ] 记录 `architecture-final`、各隔离 worktree、最新验证证据、当前 Backend Task 5b 阻塞及剩余跨计划依赖顺序，明确不把未提交草稿或兼容路径纳入集成。
- [ ] 按内容分层提交本次 Assistant、执行输出、编辑器交互、架构文档与 TODO 改动，并推送到 `origin/shadcn`。
- [ ] 为运行时输出补充结构化 `sourcePort`，同步 Rust/TypeScript execution wire、投影与契约测试。
- [ ] 统一 Output、Diagnostics、Pin result search 与 Node detail 的节点/Pin 语义化显示，并新增画布节点选择器交互。
- [ ] 接入禁用发送的 Assistant UI Workbench Shell，注册 root Dockview、布局持久化、View 菜单及中英文文案。
- [ ] 新增仓库级 `.gitattributes`，统一文本文件使用 LF 换行。
- [ ] 通过 Git 属性与 `git diff --check` 验证换行策略。
- [ ] 更新 `AGENTS.md`，明确 Rust 后端测试的编译与运行成本较高，避免迭代期间频繁运行。
- [ ] 保持 Rust 聚焦测试与完整测试套件之间的执行边界，仅在必要场景运行 broader/full suite。
- [ ] 更新 `AGENTS.md`，统一后端 Rust 错误使用 `thiserror` 定义类型化错误。
- [ ] 明确禁止以裸 `String` 传播后端错误，并在 IPC 边界统一映射为 `CommandError`。
- [ ] 完成现有 Plot、Worksheet、Info 与 Bayes 绘图链路审计，确认 D3 继续作为唯一绘图库，重构重点收敛到模块边界、数据契约、公共绘图能力和增量渲染。
- [ ] 新增 Plot 模块十二任务实施计划，固定 Rust DTO、source adapter、数据空间 `ChartModel`、typed registry 与 shared renderer 的单向数据流。
- [ ] 规划 Rust 生成的 Plot golden contract、单一 Result plot-kind guard、nullable correlation 语义和 canonical camelCase wire，防止 Rust 与 TypeScript 字段漂移。
- [ ] 规划 `shared/charts/core`、`cartesian`、`statistical` 三层目录及 theme、margin、ResizeObserver、tooltip、domain 和稳定 D3 layer 的公共契约。
- [ ] 规划 Scatter、Line、Histogram、ECDF、KDE、MultiLine、Correlation、Correlogram、DID、VAR stability 与 PredictiveInterval 的渐进迁移和旧路径原子删除。
- [ ] 明确本轮不引入 ECharts、Vega-Lite、Canvas 或万能 Chart grammar，并将 Binary margins、Bayes 诊断 authority、Worksheet backend preview DTO 与 datetime 规范化保留为独立后续计划。


## 2026.08.26

- [ ] 移除 `compiler.input.unbound` 诊断文案中的内部端口地址，仅保留结构化端口位置供界面显示。
- [ ] 为未绑定输入诊断补充回归覆盖，验证消息不携带 UUID 且 `Node · Pin` 定位仍然保留。
- [ ] 为程序输出事件补充结构化 `sourcePort`，并在 Output 中显示节点标题与端口标题。
- [ ] 让 Diagnostics、Detail 诊断位置和 Pin result 搜索统一使用 `node title · pin title`，保留 opaque ID 仅用于内部定位。
- [ ] 更新执行 wire、前端回归测试与中英文未知来源文案，并完成 TypeScript、Rust 聚焦验证。
- [ ] 收口 Project–Graph 与 Presentation Task 2A/2B 的跨计划编译顺序，保证 editor projection 先有唯一 Application owner、catalog schema mapper 后置且不产生第二条生产路径。
- [ ] 将 graph mutation/open 的 session currentness 固定为提交前 gate 与 Project authority 线性化，补齐 revision/lifecycle/operation closed errors 及 Commands-owned wire mappers。
- [ ] 强制 RecoveryRequired 的 `{ recoveryRequired: true }` 安全 detail、六项 EditorProjectionError 穷举映射与本地架构计划文档的结构/链接校验。
- [ ] 修复 graph Dockview tab 失活后重新激活时 wheel scale listener 不恢复的问题，让监听生命周期跟随 canvas 的 interactive 状态。
- [ ] 增加 graph wheel 缩放 active → inactive → active 回归测试，验证失活不响应且重新激活后恢复缩放。
- [ ] 定位 ProjectPicker「移动到回收站」前端调用链，确认确认操作进入 `delete_registered_project_files`，未误调用普通重命名路径。
- [ ] 定位 Rust 删除事务仅将项目根目录改名为 `.yssbi-deleting-<operationId>`，当前未调用系统回收站 API。
- [ ] 记录 `CleanupPending` 回执没有后续 tombstone 清理实现，导致项目列表移除但项目文件仍留在原父目录。
- [ ] 分析 editor 顶部菜单栏：当前由自定义 `WindowMenuBar` 标题栏容器承载菜单布局。
- [ ] 确认 editor 顶部各菜单项使用 shadcn/Radix `DropdownMenu` 与 `Button` 组合，而非 shadcn `Menubar` 组件。
- [ ] 确认仓库没有 `src/components/ui/menubar.*` 封装，也没有 `MenubarTrigger`、`MenubarContent` 等 shadcn Menubar API 的源码引用。
- [ ] 使用 shadcn CLI 安装官方 Menubar，并将 editor 顶部七个独立 DropdownMenu 替换为统一的语义化 Menubar。
- [ ] 保留 shadcn Menubar 默认样式与视觉尺寸，将生成的图标映射到项目既有 `react-icons/vsc`，避免引入第二套图标依赖。
- [ ] 在 `AGENTS.md` 中补充优先组合 shadcn/ui 组件及复用既有图标库的规则，并增加 editor Menubar 语义回归测试。
- [ ] 移除 editor 顶部 shadcn Menubar 根容器的外部边框，保留默认触发器、菜单项和键盘交互。
- [ ] 保留窗口标题栏自身的底部分隔线，不将 Menubar 边框调整扩散到 WindowChrome。
- [ ] 分析 root Dockview edge 收缩时仍保留 tab 激活样式的问题，确认 `collapsed` 与 `activePanel` 是独立语义。
- [ ] 确认 left/right/bottom edge 收缩时应清空 tab 的展示激活态，同时保留 Dockview 内部 active panel 作为展开后的恢复目标。
- [ ] 规划基于 root Dockview 实时 edge state 派生 visual active，避免在 Zustand 建立布局镜像，并区分 Logs nested Dockview 的内部激活态。
- [ ] 实现 root Dockview edge 收缩态 tab 的无激活视觉投影，保留 Dockview 内部 active panel 与布局持久化契约。
- [ ] 让自定义 Workbench tab 订阅 Dockview edge 收缩事件，收缩时标记 tab 为未选择，点击 tab 后由 Dockview 激活并展开 edge。
- [ ] 增加 bottom edge Output/Diagnostics 的收缩、选择和展开回归测试，并保持中心 editor 与 Logs nested Dockview 不受影响。
- [ ] 将项目删除事务改为调用系统回收站，成功后清理活动项目状态并提交注册表删除，不再在原目录旁创建 tombstone。
- [ ] 收窄删除生命周期回执：正常删除返回 `committed`，注册表失败仅返回 `registryPending`，恢复信息不再携带本地 tombstone 路径。
- [ ] 删除 tombstone/`CleanupPending` 相关死代码、UI 文案与测试，保留路径身份校验、生命周期排他及注册表失败恢复覆盖。
- [ ] 新增系统回收站成功与失败回归测试，验证成功无本地残留、失败时项目目录和注册表记录保持可重试。
- [ ] 复查六组架构设计与实施计划，统一加入 0.x replace-and-delete 约束，禁止迁移 adapter、bridge、forwarder、双路由、回退和旧新 contract 转换。
- [ ] 删除 Project 临时 tabular snapshot、Legacy Execution port、旧 Graph facade 迁移及 Frontend 新 coordinator 调用旧 writer 等设计，改为最终 owner 离线构建与单点原子切换。
- [ ] 将 Julia、ScientificBackend、Execution value/plan/resource ports 直接放入最终路径，旧 `node_system`/Project production route 在切换前不消费最终接口。
- [ ] 对齐 Project–Graph、Execution、Presentation 与 Frontend 的 Task 8 切换时序，要求同一 compiling checkpoint 切换全部 caller 并删除旧 source、tests 与 debt。
- [ ] 明确独立 canonical owner relocation 也必须一次切换全部 consumers 并删除旧声明，禁止借独立迁移建立旧 workflow 到 staged replacement 的转接层。
- [ ] 删除节点目录与编辑器投影中的节点级短 description 字段，保留 documentation 与参数级 description。
- [ ] 同步 Rust NodeCatalogProtocol、LocalizedCatalog DTO、EditorProjection DTO 及 React 目录/画布投影，避免节点详情再次读取短描述。
- [ ] 更新 Rust 生成的 node-system golden fixtures 与前端契约测试，验证节点目录和投影 wire 不再携带节点级 description。
- [ ] 复现左侧 Activity tab 二次点击无法收缩的问题，确认自定义 click handler 在展开态阻止了 Dockview 原生事件。
- [ ] 让 Activity tab 仅在 edge 已收缩时拦截点击并手动激活/展开，展开态交还 Dockview 原生收缩切换逻辑。
- [ ] 增加左侧 Activity edge 二次点击收缩回归测试，并通过 Dockview 相关测试、TypeScript 检查与前端构建。
- [ ] 分析 root bottom tab 激活态额外 margin 导致 tab 几何尺寸变化和相邻位置移动的问题。
- [ ] 将 bottom edge tab 的 margin/radius 固定到所有 tab，限制激活态只改变背景、文字颜色和语义边框。
- [ ] 增加 bottom tab 激活切换前后 computed margin 稳定性测试，并完成相关测试、构建和差异校验。
- [ ] 使用标准库脚本批量清理节点文档中的 Inputs/Outputs/Pin/Parameters 等接口说明章节。
- [ ] 为 catalog 文档清理脚本增加中英文标题识别、只读检查和显式写入模式。
- [ ] 保留节点文档的公式、模型、用法等正文，并验证批量清理不会产生额外尾部空行。
- [ ] 将共享 ContextMenu 的 item、label 与 separator 间距恢复为默认值，修复 sidebar 右键菜单分割线后的紧凑间距。
- [ ] 检查 ActionMenu、sidebar 与 ContextMenu 测试，确认仅保留行为测试，不新增或保留样式断言。
- [ ] 通过 ActionMenu 聚焦行为测试、TypeScript 检查和差异校验。
- [ ] 将根 Dockview 的固定 view tab 标题接入现有 activityBar/panel 翻译 key，覆盖底部、右侧和 Activity tab。
- [ ] 保持编辑器资源名、结果标题和 Logs workspace 内部 domain tab 的既有动态标题逻辑，不把本地化文本写入布局持久化数据。
- [ ] 增加固定 workbench view 标题的统一回归测试，并通过相关测试、TypeScript 检查和 i18n key 校验。
- [ ] 将 Workbench graph tab 的中键与右键监听从会被 Dockview 重建的 `.dv-tab` 外壳替换到复用的 React header host。
- [ ] 增加 graph tab 拆分到新 group 后仍可打开文档右键菜单的行为回归测试，并校验菜单使用新的 groupId。
- [ ] 通过 WorkbenchDockviewTab 聚焦测试、相关 Dockview 测试、TypeScript 检查和差异校验。
- [ ] 审计 Dockview tab/group 相关 DOM 监听，确认除稳定 React header host 外未发现第二处挂载到临时 `.dv-tab` 的生产逻辑。
- [ ] 发现 Workbench tab 的 edge collapsed 状态未订阅面板 group 迁移，central/edge 移动后折叠视觉状态可能滞后，需单独修复。
- [ ] 确认 Workspace 的 Delete 键事件过滤、Workbench group 订阅和 Logs nested Dockview 不属于本次 tab 监听失效问题。
- [ ] 修复 Workbench tab 的 edge collapsed 状态订阅，使面板跨 group 移动后重新绑定当前 group 的折叠事件。
- [ ] 增加 edge tab 从收缩 group 移回中央 group 后清除视觉收缩标记的回归测试。
- [ ] 修复 Logs nested Dockview 自定义 tab 未订阅 `onDidTitleChange` 导致 i18n 标题不刷新的问题，并覆盖 title event 回归测试。
- [ ] 复查 Dockview 的 title、group、visibility、collapse 和 DOM 监听生命周期，未发现其它同类生产 bug。
- [ ] 定位 Node detail 的 Pin Interface 在已连接 pin 上触发 `getSnapshot` 未缓存和最大更新深度错误的原因。
- [ ] 让 NodePinSpecRow 的 Zustand selector 只返回稳定的连接实体引用，再在组件内派生 pin view 参数。
- [ ] 增加已连接 pin 的 React 外部 store snapshot 回归测试，并完成类型检查与相关前端测试。
- [ ] 将 Detail 中原有的 Pin Interface 外层 collapsible 与 Inputs/Outputs tabs 替换为两个独立的 Inputs、Outputs collapsible。
- [ ] 保留输入输出数量和空状态提示，默认让两个类别分别收起，移除不再使用的 Tabs 状态与旧翻译键。
- [ ] 增加 Inputs/Outputs 独立展开回归测试，并完成 Detail 相关测试、类型检查和差异校验。
- [ ] 分析 Output/Diagnostics 顶部当前图路径仅为展示信息，确认可移除而不改变图级数据语义。
- [ ] 将 Output/Diagnostics 顶部栏统一为 Logs 风格的紧凑横向 header，保留标题、清理操作和诊断数量。
- [ ] 增加 Output/Diagnostics header 回归测试，验证隐藏图路径且保留 focused graph 的输出与诊断行为。
- [ ] 将 Node detail 的 Inputs/Outputs pin 内容改为统一的 name/content 两列布局，并移除旧 Pin 行的状态徽标、历史菜单和 Tooltip 逻辑。
- [ ] 为 Inputs 提供兼容上游输出选择，为 Outputs 提供可增删的输入目标下拉槽，并将选择/删除接入现有 graph mutation 命令。
- [ ] 增加 Detail pin 连接选项、输出多连接槽和精确断开连接的回归测试，完成类型检查与相关前端验证。
- [ ] 移除 Node detail Inputs/Outputs collapsible 标题右侧的数量展示及其派生逻辑，保持折叠与 pin 连接交互不变。
- [ ] 增加标题不显示数量的回归断言，并完成相关前端验证与差异检查。
- [ ] 检查 Node detail 的 Capabilities 与 Diagnostics collapsible 标题，确认存在硬编码英文文案。
- [ ] 为 Capabilities 与 Diagnostics collapsible 标题接入中英文 i18n 翻译键，不改变 capability 数据和交互逻辑。
- [ ] 增加 NodeDetailPanel 标题翻译回归测试，并完成 Detail focused tests、类型检查和差异校验。
- [ ] 清理 Node detail 测试中的旧 Tabs/标题实现细节断言及重复的输出槽操作覆盖。
- [ ] 保留 Inputs/Outputs 兼容选择、输出槽增删、精确断开和输入清空的最小回归覆盖。
- [ ] 在 graph canvas 节点右键菜单中增加临时“选择节点…”入口，保留原生右键菜单的键盘导航行为。
- [ ] 为节点选择器实现当前节点/首节点初始化、上下箭头切换和 Enter 确认选择，并完成编辑器回归验证。
- [ ] 修复 graph canvas 节点选择列表 selector 创建新对象数组导致 React 外部 store snapshot 不稳定的问题。
- [ ] 为节点选择选项 projection 增加稳定引用回归测试，并完成 React 19 更新深度错误验证。
- [ ] 复核 `compiler.input.unbound` 的结构化端口位置与文案参数，保留 `Node · Pin` 定位能力并恢复 `{port}` 上下文。
- [ ] 恢复未绑定输入分析阶段传入精确端口地址，同步编译器诊断定义与 lowering 回归断言。
- [ ] 完成 compiler diagnostics 与 lowering 聚焦 Rust 测试验证。

## 2026.08.25

- [ ] 将 Logs 内嵌 Dockview 子 tab 固定为完整日志域集合，移除 tab 关闭 X 和中键关闭行为。
- [ ] 删除 Logs 右侧“+”新增日志域菜单及相关新增逻辑，缺少固定域的持久化布局自动回退到完整默认布局。
- [ ] 限制 Logs tab 仅可在同一组内交换顺序，并优化激活态、悬停态和右侧工具栏样式。
- [ ] 增加 Rust production-module architecture audit，强制生产 Project 不依赖 Application 或 Commands，并对 raw identifier、conditional path 与代码 include 采取可验证的保守处理。
- [ ] 将 Project-relative DuckDB runtime binding/physical removal 下移 Database，将现有 ColumnInfoDTO conversion 下移 Schema，保持事务、快照、错误与 IPC 行为不变。
- [ ] 更新权威架构图与 Database module 文档，明确 Application 编排、Project authority、Database primitives 和 Schema wire conversion 的单向边界。
- [ ] 执行 strict architecture policy：用 Rust/TypeScript canonical-origin 审计、exact debt 与 semantic guards 强制单向依赖，并把现有债务逐项清零。
- [ ] 执行 Rust backend adapter boundaries：让 SCI、Database、watcher/progress 和 scientific/relational/resource ports 脱离 Graph、Project、Tauri 与具体后端。
- [ ] 执行 Project–Graph ownership decoupling：Graph 只拥有 document/schema/catalog/compiler contract，Project 保持唯一持久化与 history authority，Application 负责 capture/plan/commit。
- [ ] 执行 Execution runtime extraction：建立原子 Application session、Execution-owned plan/runtime/settings、RunRegistry 与两阶段 finalization，删除 Project 执行 owner。
- [ ] 执行 Presentation/Event/Command boundaries：把 editor/result presentation 与跨域事件策略归 Application，Schema/Event 只做 wire/delivery，Tauri commands 保持薄层。
- [ ] 执行 Frontend Application boundaries：后端状态只作不可变 projection，Application hooks/coordinators 统一 reconciliation、optimistic echo 与 use-case，UI/store 不再直连 Services/Tauri。
- [x] 将 Canvas 编辑器交互按 `panelInstanceId` 隔离，修复同一 group 中多个 tab 共享 active tab 导致的右键创建节点、选择、连线、拖放和快捷键操作失效问题。
- [x] 清理 Canvas 的 group 级 active tab、重复命令路由、失效拖放处理和旧的自定义 tab 移动逻辑，统一使用 Dockview 的默认移动行为。
- [x] 删除与上述历史实现绑定的冗余测试和失效测试；保留必要的 pane 快照稳定性约束。
- [x] 修复空 panel 选择状态返回新对象导致 React `getSnapshot` 无限更新的问题，使用稳定的空选择快照。
- [x] 修复首次进入项目时左侧 Activity sidebar 默认激活 `commands` 的问题，默认改为激活 `project`。
- [ ] 将现有双语节点 Markdown 以编译期 catalog 文档 registry 接入 Detail 的 documentation projection。
- [ ] 让静态节点与资源绑定节点按当前 locale 读取 Markdown，并在语言缺失时回退英文。
- [ ] 对没有显式 Markdown 映射的节点停止使用 i18n documentation 作为隐式 fallback，保持文档单一来源。
- [ ] 将 `PortSpec.title` 固定为各节点结构化 pin 定义直接提供的非本地化标题。
- [ ] 删除协议层按 `nodeType`/`key` 推导 pin title 的全局映射、特例分支和未知 key fallback。
- [ ] 保持 Markdown 中的 pin title 与 Rust 结构化 pin 定义人工同步，不在运行时或编译期解析 Markdown title。
- [ ] 按 ponytail 审核 pin title/Markdown 相关测试，移除 catalog 职责之外的重复文档断言并保留稳定的 projection 契约。
- [ ] 更新 node-system golden fixtures 以反映 Markdown documentation 与结构化 `PortSpec.title`，移除已失效的 `label_key` 快照契约。
- [ ] 合并 Markdown documentation 的正向验证到 catalog 公共 projection 测试，覆盖英文、中文与 locale 差异。
- [ ] 移除 DataFrame 参数化节点测试中与节点参数契约无关的 documentation 断言。
- [ ] 重新生成 node-system golden fixtures，使 Markdown documentation 与 `PortSpec.title` 成为当前契约。
- [x] 使用的 ag-grid ~~我想将 @glideapps/glide-data-grid 切换为 shadcn 中的 data table，主要是因为风格和组件和目前的 shadcn 组件不搭，同时在构建的时候还有一些其他的错误，如下。需要考虑替换的可行性；或许使用 Handsontable 替代（商用收费）~~
- [x] tolerance 和 num_traits
- [x] 在前端中的 graph 中的 data pin 的类别都是 unknown，导致节点没有颜色，同时在 pin 的时候不会筛选节点，更不会自动连接节点，这个是需要修复的，可能需要完整的从后端发送类型过来避免字符串解析？这样会更加完整？这里需要仔细考虑
- [x] 将过去的操作尽可能实现后归纳到 v0_0.md 文档
- [ ] node 的 tooltip 功能，可以查看节点的信息
- [ ] 在根 Dockview 的 Output 右侧新增 Diagnostics tab，集中展示当前图的节点诊断信息。
- [ ] 按当前聚焦图的节点顺序汇总所有节点 diagnostics，并显示严重级别、节点、诊断 code 与消息。
- [ ] 点击 Diagnostics 条目后定位对应节点并切换 Details 上下文；旧布局恢复时自动补齐缺失的 Diagnostics tab。
- [ ] 移除 editor 顶部主题切换按钮右侧的 Details 切换按钮及其 menubar/View 菜单逻辑。
- [ ] 将 Details 固定为根 Dockview 右侧常驻 sidebar，初始化、旧布局恢复和重置布局时自动创建并默认展开，同时保留用户调整的宽度。
- [ ] 保留 Details Dockview tab 原有图标与文本标题样式，仅移除关闭入口，不改变 Activity bar 的纯图标样式。
- [ ] 禁止 Details 通过关闭、上下文菜单或拖拽布局离开右侧 sidebar，并保留 Details context 更新功能。

架构，不要你中有我我中有你，最好组件化？是这个意思吧？即下面的分析

- [ ] snapshot 有必要吗？？？？ 还有 run id，以及每次允许之后会在 details 中出现的 developer trace 中记录的历史数据，打开会很卡。
- [ ] 在更改 graph 的时候 tabbar 中的样式并没有其他变化，如果在更改后不保存关闭，那么下次打开打开的时候还是更改前的状态，这里明显是不符合逻辑的，除此之外还有其他的需要检查；同时磁盘上以及更新的符号和标签我感觉可以去掉，可以学习 vscode 的 tabbar 处理
- [ ] 在 sidebar 中创建 item 的时候首先会出现在最下方然后根据 name 移动位置，能不能直接根据 name 出现在某个位置，忽略出现在下方的过程，这样不美观
- [ ] 目前后端节点的定义好像也不太清晰明了，需要讨论怎么处理
- [ ] 在 graph 中的右键菜单我希望根据 section subsection 等等分类，包括 activitybar 为节点的 sidebar 中的节点也是一样，这样如果不记得名称找起来非常方便，需要讨论
- [ ] 后续我会加入 mcp 功能方便 llm 直接调用统计方法获得数值结果，同时我会加入智能分析功能利用 llm 分析数值报告获得分析结果，在这里我初步的构想是在 activitybar 中添加一个报告的 icon，其对应的 sidebar 中显示各种数值报告的 item，然后 llm 可以对这些报告进行执行分析输出得到结果编写论文；你怎么看，目前怎么预留接口，前端应该如何设计等等需要仔细讨论和实现（在这里 data 中每一列我希望可以添加一个描述统计，意味着我们可以在导入数据的时候对 data column 添加一个文本标注，方便模型知道 data column 并进行描述统计分析一些有意义的结果并输出一些合理的假设）
- [ ] 关于可视化层面，目前可视化的图表还不够，我希望更加的丰富；并将这些图表组件化放置在一起，哪里需要就调用避免重复实现，差异较大可以分为两个组件
- [ ] 思考是否有必要多窗口进行跨窗同步，这样就不需要什么多进程的 token 了吧
- [ ] graph 分为两种，一种是纯计算 graph，一种是目前这种；纯计算 graph 使用 notebook 这种形式，修改节点会污染依赖该节点的下游节点，递归污染；运行到此节点可以做到将上游阶段全部干净，
- [ ] 我认为下面的版本信息完全没有必要


            GlobalVariableMutation::Delete {
                id,
                expected_revision,
            },


## v1.0 待办

`v2` 原本用于区分旧持久化格式，但既然明确要求**不迁移、不兼容、直接删除旧路径**，继续维护版本号没有价值，反而暗示未来会做 schema migration。

### 窗口跨窗同步

- [ ] assistant-ui
- [ ] 将 worksheet 重命名为 charts

```
"/*#__PURE__*/"

in "node_modules/.pnpm/@glideapps+glide-data-grid@6.0.3_lodash@4.18.1_marked@4.3.0_react-dom@19.2.7_react@19.2_c19a5bde3a2383671a6324b7c97614b7/node_modules/@glideapps/glide-data-grid/dist/esm/internal/data-editor-container/data-grid-container.js" contains an annotation that Rollup cannot interpret due to the position of the comment. The comment will be removed to avoid issues.
node_modules/.pnpm/@glideapps+glide-data-grid@6.0.3_lodash@4.18.1_marked@4.3.0_react-dom@19.2.7_react@19.2_c19a5bde3a2383671a6324b7c97614b7/node_modules/@glideapps/glide-data-grid/dist/esm/internal/data-grid-overlay-editor/private/markdown-overlay-editor-style.js (2:13): A comment
```
### 口语化表达

- [ ] **多数据库 DataView 直接编辑行定位抽象**：当前项目内 DuckDB 持久化表用 DuckDB `rowid` 做分页/编辑定位；后续若支持 SQLite / MySQL 等外部数据库直接编辑，需要新增 `RowLocator` / `BackendRowKey` 类能力抽象，各 backend 明确自己的稳定行键策略（DuckDB `rowid`、SQLite `rowid` 或主键、MySQL 必须主键/唯一键）；无稳定行键的外部表默认只读或先导入项目 DuckDB，避免把 DuckDB `rowid` 语义错误泛化到所有数据库
- [ ] **Worksheet 图表切 tab 性能优化（坚持 ChartViewModel 路线）**：不要全局把所有 tab 内容 hidden 保活；继续沿用当前 preview/data 缓存方向，把昂贵工作从 React mount 生命周期中移出。后续将 `WorksheetPreviewPayload` 细化为更完整的 `ChartViewModel`（缓存数据列、聚合结果、domain、ticks、legend/tooltip 元信息等），组件重挂载时直接复用模型；绘制层避免 `svg.selectAll('*').remove()` 全量重建，尺寸变化只重算 scale/位置，大数据 scatter/line 考虑采样或 canvas 渲染；缓存使用 LRU，并在 DataView 编辑、数据版本变化或 worksheet spec 变化时精确失效
- [ ] **变量类型切换时的值迁移 / 智能转换（暂缓）**：当前策略——切换类型且未显式提交新值时，重置为 `DataType::default_value()`；Array / Object / DataFrame / DataSeries 已用 JSON 列式编辑 + `tabular/` 存储（见 ## 2026.07.03 已勾项）。**暂不实现**跨类型自动保留或 coerce（如 Int→String、DataFrame↔Array、Object 字段映射等）；后续可考虑接入 `DataValue::coerce_to`、切换前「将丢失当前值」提示。变量类型不可选 Any。
- [ ] **View：大 Array 分页 tabular（暂缓，没有合适的前端表现）**：一维、同质、较长 Array 走后端 `getPage`（2 列 `#` / `value`，与 DataSeries 同 API 形状）；短数组 / 嵌套 / 异构仍走 `json` + `JsonTreeView`；需 `ResultSource` 或虚拟 tabular 存储与 builder 分支。
- [ ] **Struct handle → View JSON 架构重设计（暂缓实现）**：当前 `ExecutionDataStore` 仅存 `Arc<dyn Any>`，`DataValue::Struct` 的 `typeKey` 与 handle 分离；View / `build_struct_source` 只能事后按 `typeKey` downcast，临时用 `execution/struct_json.rs` 中央 match 表（OLSModel、OLSResult 等逐个注册）——每增 Struct 要改表，且 `typeKey` 双写，不可持续。**待选方向（均未定稿，先不实现）**：① **入库 JSON 快照**：`put_struct(type_key, T: Serialize)` 写入 handle 时同步 `view_json`，View 只读快照、Predict 仍 downcast；② **`dyn ViewPayload` trait**：handle 自带 `view_json()` + `as_any()`；③ **TypeId 注册表 + macro**：注册点贴近类型定义，替代 central match；④ **View 永不碰 handle**：所有 output 注册 source 时必须带 JSON（仍要解决无 upstream source 时的首次序列化）。实施前需统一：handle 层 vs `ResultSourceStore` 谁为 JSON 真源、不可 `Serialize` 的类型（如 `StandardizeTransform1D`）策略、与现有 `source_id` 复用链如何衔接。完成后删除 `struct_json.rs` 式 per-type 注册。
- [ ] **View 节点展示（续）**：核心 renderer / source 统一 / 子窗口 layout 已完成，见 ## 2026.07.03 未完成项（Array 分页 tabular、子窗口 chrome、runtime source 生命周期、Struct handle JSON 架构重设计）。
- [ ] 复制粘贴撤回逻辑的快捷键效果有问题
- [ ] 值类型处理
- [ ] 还有 7 个组件属于「壳统一了、内部还没拆干净」，优先级建议：VEC → Panel → DID → VARSoc → DFADFSummaryList → VecRank → DFADF。
- [ ] 优点：window_* 是「当时那一刻」的不可变快照，重跑不会误改已打开窗口里的内容。代价：不关窗时会累积（For 循环多次 View 会留下多个 window_*），直到关窗或 clear_all。文档里提过 Window LRU/TTL，尚未实现。
- [ ] **On Error / 错误传播（待设计）**：MaxIterations + loop_counters + 执行前清空已落地。错误模型仍停在「节点失败 → 记日志 + 发事件 + 整图 has_error」，没有可连线的错误传播；要做 On Error 需先定：错误是否中断下游、是否进专用 exec pin、与 Loop/Sequence 如何交互等，再扩 `ExecutionEffect` 和 executor。
- [ ] 节点样式问题
- [ ] **OLS 取数「逐边」vs「批量」语义文档化**：当前执行器按边 `emit_data_pull` → 求值 → `emit_data_flow`；确认是否故意取代旧 NodeStart 批量高亮，并在 `TODO`/执行器注释中写清 UX 预期，避免后续误改回批量形式。
- [ ] uistyle 可能需要根据节点类型来进行重构
- [ ] 在 editor group 多个的情况下，刷新后回到了单个 watermake 界面，但是同时会出现警告：当前编辑器图未能加载，请重新点击标签页或画布
- [ ] 函数图层中 **递归 Call 编辑器提示**：`CallDepthGuard`（64）仅 runtime 报错；编辑器内对自递归/深链 Call 做静态提示（非阻断），与超限单测（见 Rust 复盘）配套。
- [ ] sidebar 内容中的 scrollbar 以及日志及其他组件内容的拖动逻辑有问题
- [ ] 剩余唯一标记是 Rust 执行上下文中的 get_bound_type TODO。它依赖尚未提供类型绑定状态的 GraphRuntime，当前直接返回 None 是明确的未实现能力，不适合通过猜测补丁，否则可能引入错误类型推断。
- [ ] **ACF/PACF 命令与 Plot 节点 DTO 对齐**：`plot/correlogram.rs` 输出 `CorrelogramDatum { lag, value, q_stat, p_value }`；`command_sci::compute_acf_pacf` + InfoView `ACFPACFBlock` 仅 `Vec<f64>` + `n`——复用 `cumulative_ljung_box`，扩展 `AcfPacfResponse` 或共用 `CorrelogramPlotData`，避免 Summary 图 tooltip 缺 Q/p-value（前端 `CorrelogramChart` 已按可选字段防御）。
- [ ] **Julia 第二个迁移目标选择**：ACF/PACF 已经有 `src/sci` API、Julia worker 和 Rust/Julia golden fixture 测试；下一步不要直接上 VEC/RE MLE/DID。优先在「serial tests / Ljung-Box / DW」和「描述性统计」里选一个做第二个 PoC：输入输出简单、能复用 Arrow IPC、容易与 golden result 对齐。简化 OLS 可以排第三步，先只做 `y: Float64` + `x: Float64 matrix` + `hasIntercept`，暂不碰公式、分类变量、robust/cluster/HAC。
- [ ] bayes 中的有很多的 errors.push(error("PREDICTOR_REQUIRED", "预测表达式尚未解析或绑定。", "boundPredictor")); 后期都是要修复的
- [ ] bayes 中的 ast 感觉可以和 src 下的 ast 放置在一起，在这里好像有 latex -> json ast，json -> julia ast，normal formula -> json ast 等等 ast
- [ ] bayes 长任务的通知最好是作为复用模块
- [ ] Failed to install Juliaup: 找不到与输入条件匹配的程序包。安装不了 julia
- [ ] 在这里似乎日志类的测试感觉没有必要，可以直接删掉
- [ ] clippy::too_many_arguments 这些感觉需要清理，不符合 rust 代码标准

函数和事件保持一致性的 API 重复层面：不影响编辑一致性，但维护成本高：

useGraphManagement 里 addEvent / addFunction、deleteEvent / deleteFunction 几乎镜像，底层已是 createGraphResource(kind) / deleteGraphWithConfirm(kind)
GraphResourceKind vs GraphResourceType 两处 type alias（sidebar / editor）
快捷键 Ctrl+N 仅新建 Event（产品选择，非 bug）
Menubar / Watermark 仍分「新建 Event / 新建 Function」两项（入口文案差异，合理）
若要进一步收敛，可以把 Session 对外 API 收成 addGraph(kind) / deleteGraph(kind)，Sidebar/Menubar 只传 kind，不再暴露四套函数名。

# TODOLIST

zed 的 .rules 文件需要学习，同时还有根目录中的内容，有必要学习一下

"lint:rs": "cargo clippy --manifest-path src-tauri/Cargo.toml --workspace --all-targets --all-features -- -D warnings", 这里的 -D warnings 后续肯定需要处理掉

当前 `pnpm run ci` 尚未全绿，原因均为既有基线：

1. Oxfmt 检测到 1592 个文件尚未建立格式化基线。本次没有运行 `pnpm format`，避免产生全仓批量格式化改动。
2. 严格 Clippy 在 `yss-sci` 中报告 84 个 library、86 个 library-test 既有 `-D warnings` 问题。
3. 主工作区前端测试为 1830 通过、1 失败；失败测试
   `src/features/core/dockview/workbenchDockviewDefaults.test.ts:115` 的期望值缺少现有的 `assistant: "right"`。
4. 默认 Vitest 仍会扫描嵌套的 `.worktrees/`；本次完整主工作区验证通过 CLI 临时排除了该目录。


绘图组件库需要重构

xt align 进行对齐，在这里是不是可以于 ts align 可以共用呢

感觉 faer 计算得很慢呢，是不是没有开启并行的原因，4500*17 维度需要耗时 1600ms 有点儿久

然后就是真的需要 polars-dtype 这个 crate 吗？我感觉 polar 应该包含了吧， ai 是不是弄错了

polars 的 csv 有意思，with_try_parse_dates 可以检索日期，没有检索 categories 的

现在节点分类特别混乱

gls 的 data input pin 中我认为可以设置为 matrix，这就意味着需要在值系统中添加并定义 matrix 类型，目前是 dataframe

wls 和 gls 的 predict 节点有问题，目前 wls 报错：Node 6b7c0693-8d92-4c76-a253-6d49333221ab failed: Predict: Model input is not connected or invalid

HAC 已实现（Bartlett、Parzen、Quadratic Spectral kernel，lag 参数）；hac-panel、hac-groupsum 尚未实现。

目前使用 fixed scale 并没有提供 scale pin ，同时在 ols configure vce 可接受的结构体中删除掉 hac-panel 和 hac-groupsum

对了 schema 中关于 vce 的要删除，暂时没有用到

OneOf 还是使用 Restriction + TypeVar 来做类型推断之类的东西？

oneof 类型连接其中一个类型的 pin 的时候，会变成该类型；（这个效果是好还是不好？很难知道）

我觉得 plot 可以学习 seaborn 来进行参数选择和绘制

catelog 中的 plot（尤其） 和 distribution 中的内容需要大量重构

同时 plot 可以使用数据节点，然后数据节点需要使用 plot 的 show 节点才可以展示，然后 show 节点可以结合多张图的数据节点，在新的窗口中配置如何结合的信息。

复制粘贴节点以及项目的保存不够完善

直方图有问题，其显示将 null 值当作了 0 在图中处理，正常应该忽略

打开 dataviewer 后，在 editorviewer 中导入数据，dataviewer 并没有更新；

类型推断系统不够完善

type infer 使用 dirty 的形式推测，不要每次都推测全量类型

function, macro 功能添加（感觉没必要区分 function 和 macro ）

ols 节点的 evaltor 逻辑有巨大的优化空间

不如纯 data 节点在连接的时候就进行计算就好了

执行动画有一点bug，节点在获取数据执行其他节点的时候本身状态并没有执行完毕但是ui上显示执行完毕了

为了解决 dummy 问题，例如个体效应和时间效应的哑变量问题，可能需要在 dataseries 中添加额外信息进而去提出多出来的哑变量：计划做一个 add dummy info 节点，下方有个下拉列表或者文本框选择需要设置 dummy 的信息，仅对 dataseries 为 string 的信息管用。

面板数据，可能需要对 dataframe 数据类型也加上 info，来表示个体信息和时间信息 dataseries

未来可视化需要加上地图，地图应该使用下载的方式下载到数据库中，下载完毕后才能使用；

settingview.tsx 中，要注意区分，首先 dataframe 是必须要设置形状和颜色的，然后 array, dataseries 等复合类型应该是控制形状，基础类型控制颜色，同时基础类型应该是相同的形状，any 和 受限制的 any 应该是颜色，应该区分 all any, base any, part any

工具变量法：2sls

dataframe 抽样方法，是在 ols 配置还是 dataframe 层面配置呢

参数网格 -》 数据变换（异常值检验，对数变换等等操作） -》 检验结果 -》 存储

ols_summary 打开 ols_result_viewer 并返回 ols_result, 这里面存储了一些统计模型信息可以使用节点进行提取；这些节点应该是使用类似于函数或者其他功能的注册的方式而不是定义，不然后续结构体太多了这里会爆炸

ols model 可以引申出一个新的节点 predict，这个节点可以使用 endog, exog 两个玩意获得拟合值，然后真实值 - 拟合值可以得到残差。这是基本操作不应该删除

下面这玩意在软件退出时保存了两次

[12:43:12.734][BE][INFO] [APP] Settings loaded successfully via backend
[12:43:12.734][BE][INFO] [APP] Settings loaded successfully via backend
[12:43:12.735][BE][INFO] [APP] Settings loaded successfully via backend
[12:43:12.736][BE][INFO] [APP] Settings loaded successfully via backend
[12:43:12.756][BE][DEBUG] [APP] Settings saved successfully via backend
[12:43:12.757][BE][DEBUG] [APP] Settings saved successfully via backend
[12:43:12.757][BE][DEBUG] [APP] Settings saved successfully via backend
[12:43:12.758][BE][DEBUG] [APP] Settings saved successfully via backend


需要你确认的执行语义

我建议选择：

### A. Demand-driven（推荐）

- 普通运行只计算终端结果、effect 和跨 island 依赖；
- 中间 Pin 不自动物化；
- 点击 Pin 预览时单独请求该输出；
- compiler 根据 requested outputs 决定 roots。

### B. 所有 Pin 每次都可立即查询

- 每次运行自动计算并保存所有节点输出；
- 行为接近旧架构；
- relational pushdown 和大图优化空间明显受限。

### C. 用户配置

- 默认 demand-driven；
- 可将特定 Pin 标记为“始终保留”；
- 灵活，但第一阶段复杂度更高。

请选择 **A、B 或 C**。我推荐 **A**。

## 2026.08.29

- [ ] 完成 architecture decoupling 的最终 Rust production composition：ApplicationSessionSlot
  注入 scientific/resource/Bayes/artifact adapters，Graph neutral package 进入 Execution，
  旧 Project/node-system compiler、projection、command route 与重复测试 owner 已删除。
- [ ] 完成 Backend Task 5b、Database session/query ownership、Project–Graph Tasks 2–10、
  Execution Tasks 8–9 与 Presentation/Command Tasks 4–8 的生产 caller cutover；Rust debt
  manifest 已清空并由 exact architecture policy 维护。
- [ ] 完成 Frontend Application atomic cutover：ProjectSync、Database/Variable/Worksheet/
  Result/Bayes/Window actions、Dockview read/control/root binding 与 UI/settings capability
  已收归 Application/Core 合约，Frontend architecture debt 已清空。
- [ ] 维护 architecture、backend-adapter、Project–Graph、Execution、Presentation/Command、
  Frontend Application 与 Workbench 文档，使实现地图不再描述 staged/旧 production route。
- [ ] Rust 测试链接成本高；增量阶段只执行 fmt/check，待全部 cutover 完成后统一执行一次
  Rust architecture/runtime/SCI 验证批次，并记录最终命令输出。
- [ ] 最终批次记录：`pnpm verify:full` 已在 architecture-final 通过；Frontend 311 files / 1755
  tests、yssbi library 509 tests、database integration 19 tests、yss-sci 全部 suites 与
  `git diff --check` 均通过。该条仅作为 append-only 完成记录，后续 Rust 验证继续按昂贵批次
  约定执行。
- [ ] Project→Application boundary cleanup 完成记录：Project production reachability audit
  保持无 Application/Commands 依赖，Database project storage 与 schema wire conversion 使用
  最终 owner；相关计划 checklist 与维护文档已同步。

## 2026.08.30

- [ ] 修复 Application Zustand 绑定器的 selector 快照稳定性，避免对象 projection 在 React 19
  `useSyncExternalStore` 中触发无限更新。
- [ ] 将独立 `yss-sci` crate 迁移至 `src-tauri/crates/yss-sci/`，并同步 Cargo workspace、
  架构审计 fixture 与维护文档中的路径。
- [ ] 清理已完成的迁移边界文档与空债务豁免机制，让前后端架构发现项直接触发失败。
- [ ] 收敛本地工作流与 Workbench 验证文档，移除失效命令、已删除 benchmark 和易漂移的测试文件库存。
- [ ] 将 Rust `tracing` 采集、过滤、脱敏及 console/rolling JSONL 输出迁入独立
  `src-tauri/crates/yss-tracing/`；Diagnostics 仅保留 sanitized Rust projection、frontend
  ingestion、recent ring、sequence 与 live delivery，并由架构审计守卫两层边界。
- [ ] 将 persisted data contract 迁入独立 `src-tauri/crates/yss-data-contract/` Pure Leaf，
  由该 crate 唯一拥有 `DataType`、`DataValue` 与 wire compatibility tests，并移除主 crate
  中的兼容 re-export。
- [ ] 将 persisted database contract 迁入独立 `src-tauri/crates/yss-database-contract/`
  Pure Leaf，由该 crate 唯一拥有 declaration、engine/session identity、observation 与
  fingerprint，并统一拥有 CSV/Parquet export format 与严格 parser；消费方直接依赖该 crate，
  不保留主 crate 兼容 re-export。
- [ ] 将有序 tabular contract 迁入独立 `src-tauri/crates/yss-tabular-contract/` Pure Leaf，
  集中 wire/shape invariants；Polars materialization 与 variable normalization 继续由各自
  adapter/application owner 持有，主 crate 不保留兼容 module。
- [ ] 将 Polars materialization 与 JSON-to-`AnyValue` 严格转换迁入独立
  `src-tauri/crates/yss-tabular-polars/` Backend Adapter；删除根 `backend_adapters::tabular`
  owner、测试 module 与兼容 facade；统一 database/Bayes 的 scalar/column conversion，保留完整
  `u64`，修复 1970 年前时间戳投影，并移除从未构造的 `UnsupportedColumnType` 分支和零调用的
  `database/row_mapping` 重复实现。
- [ ] 将 Polars-backed typed IPC/CSV/Parquet filesystem I/O 迁入独立
  `src-tauri/crates/yss-tabular-io/` Database Core；删除根 `database/tabular_io` owner/facade，
  四个 database/Bayes 消费方直接依赖 crate，并允许不带目录的相对输出路径写入当前目录。
- [ ] 将 dataset profile DTO 与内存 Polars 统计迁入独立
  `src-tauri/crates/yss-dataset-profile/` Database Core；DuckDB physical profiling 保持
  engine-specific 并直接构造同一 DTO；删除 `column_stats`、`column_distribution`、
  `dataset_overview` 三个根 owner/facade，统一非字符串投影与同频排序，过滤非有限直方图值，
  并用饱和运算保护总单元格计算。
- [ ] 建立 `src-tauri/crates/yss-duckdb/` Database Core engine crate，先迁入 DuckDB
  identifier/literal quoting、editable type allowlist 与 dataset-profile physical SQL；删除根
  `duckdb_sql`/`duckdb_analytics` owner 与 re-export，并由根 runtime 通过借用型列 metadata
  视图直接调用；typed CSV/Parquet `COPY` export 也归入同一 crate，Loaded DataFrame 直接调用
  `yss-tabular-io`，删除混合职责的根 `database/export` owner；后续 reader/editing 继续向同一
  crate 收敛，避免形成 DuckDB 微型 crate 群。
- [ ] 将 persisted variable model 迁入独立 `src-tauri/crates/yss-variable-contract/`
  Pure Leaf，由该 crate 唯一拥有 `VariableId`、`VariableScope` 与 `VariableInstance`；
  application/project 仅持有 mutation、normalization 与 authority。
- [ ] 将受限数学表达式 IR、plain/LaTeX parser、关系拆分与输入预算迁入独立
  `src-tauri/crates/yss-math/` Pure Leaf；根 crate 消费方直接依赖该 crate，不保留
  `math` 兼容 module，并收敛 `mathlex` 为单一声明与单一使用层。
- [ ] 将窗口 kind/default、后端权威几何缓存、主窗口恢复与原子 JSON 持久化迁入
  `src-tauri/crates/yss-window-state/` Platform Adapter；移除根 `window_state` owner，
  并以 typed error 保留持久化主错误与临时文件清理失败。
- [ ] 将 sanitized Rust log projection、frontend ingestion、recent ring、sequence 与
  bounded live delivery 迁入独立 `src-tauri/crates/yss-diagnostics/`；生产构建仅单向
  依赖 `yss-tracing`，通过正式的 platform-neutral batch sink 测试而不暴露 dispatcher
  测试后门，并且不保留根兼容层。
- [ ] 将稳定 graph node/port/type/schema/value protocol、wire validation 与 dataframe
  nominal literals 迁入独立 `src-tauri/crates/yss-graph-protocol/` Pure Leaf；消费方直接
  依赖 crate，catalog assembly 测试归还 Graph owner，根 crate 不保留兼容 module。
- [ ] 将 persisted graph document、entity identity 与 graph resource path 迁入独立
  `src-tauri/crates/yss-graph-document/` Pure Leaf；删除根兼容 module，并让 resource path 直接
  消费独立 resource-name contract，避免镜像错误类型和重复规则事实源。
- [ ] 将跨 graph/worksheet 的严格文件资源名校验、Unicode portable key 与冲突分配迁入
  `src-tauri/crates/yss-resource-naming/` Pure Leaf；`yss-graph-document` 与 Project 直接消费该
  crate，不保留 graph/root 兼容 re-export。宽松数据库/变量显示名的 `1`/`_1` 兼容语义保持
  独立，避免以表面去重改变持久化命名行为。
- [ ] 将数据库/变量宽松显示名的大小写敏感冲突分配迁入
  `src-tauri/crates/yss-display-naming/` Pure Leaf；删除根 `project/unique_name` owner 与兼容路径，
  以无正则单遍解析保持 `base N`/`base_N` 共享编号及从 `1` 开始的既有语义，移除调用方临时集合，
  并删除零调用的前端 `getUniqueName` 重复事实源。
- [ ] 删除无生产调用的根 `graph/value` 层，不为 dead code 创建 `yss-graph-value`；移除两套
  漂移的 `DataType` accept/convert/value-type 规则，并由架构测试禁止兼容目录回流。
- [ ] 将 canonical JSON 域分隔哈希迁入独立 `src-tauri/crates/yss-canonical-hash/` Pure Leaf；
  registry、analysis 与 runtime 直接依赖该 crate，并删除 registry 中重复的手写 SHA-256 实现。
- [ ] 将 provider/type/category/node registry、校验与 fingerprint 迁入独立
  `src-tauri/crates/yss-graph-registry/`；删除根兼容 module，并清除 nominal prepared-value/no-op
  lowerer、永久禁用 legacy lowerer、无调用 snapshot helper 与重复 capability API。
- [ ] 将 analysis snapshot、semantic graph、diagnostic、basis 与 provenance 迁入独立
  `src-tauri/crates/yss-graph-analysis-contract/`；删除根兼容 module，并清除零调用的 test-only
  resource resolver、projection/alias helper、永久禁用 compatibility projection 路径与重复
  unknown/blocking API。
- [ ] 将 compiler diagnostic code、双语模板与定义校验迁入独立
  `src-tauri/crates/yss-graph-compiler-diagnostics/`；删除根兼容 module，并清除仅由自身测试调用的
  diagnostic 构造、排序、role/scope helper 与 tracing/UUID 依赖。
- [ ] 将 built-in protocol、localized catalog 与内置节点文档迁入独立
  `src-tauri/crates/yss-graph-catalog/`；删除根兼容 module、未挂载且重复 resolver ID 的
  `project_interface.rs`、漂移的永久禁用 plot tests、零调用 fault/mutator API 与无效导入；根库测试入口通过
  `test-support` feature 显式隔离。
- [ ] 在提取 `yss-execution` 前删除仅由 `cfg(test)` 挂载的旧 `node_system` runtime 与
  `execution::plan::legacy`，同步清除 ProjectStore 中生产不存在的 database/catalog/run/result
  镜像、永久禁用旧 command/application route 及其测试，确保新 execution 成为唯一运行时事实源。
- [ ] 将唯一生产 execution runtime、plan、ports、result store 与 lifecycle state 迁入
  `src-tauri/crates/yss-execution/`；根 crate 直接消费该 crate 而不保留兼容 module，测试构造器通过
  `test-support` feature 隔离，并删除从未读取或构造的 candidate/resource effect 镜像链。
- [ ] 将 Graph 编译资源标识、函数/变量 contract、数据库 schema 与 immutable resource catalog
  snapshot 迁入 `src-tauri/crates/yss-graph-resource-contract/` Pure Leaf；根 crate 不保留兼容
  module，并以显式文档和架构门禁防止其与 built-in `yss-graph-catalog` 漂移成重复事实源。
- [ ] 将 Graph document analysis、editor projection facts 与 result category 判定迁入
  `src-tauri/crates/yss-graph-analysis/`；根 crate 不保留兼容 module，并删除从 Project settings/
  resource catalog 构造后仅被丢弃的 no-op analysis 输入链。
- [ ] 将 neutral Graph lowering、immutable compiled package 与 compile error 迁入
  `src-tauri/crates/yss-graph-compiler/`；根 crate 不保留兼容 module，并删除恒为 `Some` 的
  compilation report、空 diagnostics、重复 basis 与零生产者 error 分支。
- [ ] 将 Graph document invariant、atomic patch、candidate staging 与 edit error 迁入
  `src-tauri/crates/yss-graph-document-edit/`；根 crate 不保留兼容 re-export，并删除零调用的
  `address_is_complete` helper，保持 `document → document-edit → editor → runtime` 单向依赖。
- [ ] 将 persisted `DataType` 到 Graph `TypeExpr` 的 canonical typed conversion 迁入
  `src-tauri/crates/yss-graph-type-mapping/` Pure Leaf；editor/runtime 直接消费该 crate，并由架构
  门禁禁止两套逐 variant 映射表回流。
- [ ] 将 editor mutation、连接兼容性与 portable subgraph codec/instantiate 迁入
  `src-tauri/crates/yss-graph-editor/`；删除根 `graph/document`、`graph/compatibility`、
  `graph/mutation` 与空 `graph/node` owner，收敛端口解析和类型校验为单一事实源，并清除永久禁用的
  projected/materialization/revision-store 兼容路径；资源参数绑定只读取 registry protocol，兼容目录
  过滤不再在 runtime 复制端口推断；Application 仅保留 catalog snapshot 编排与 session-revalidated
  graph commit seam。
- [ ] 将 session-scoped registry/catalog 组合、analysis、open candidate materialization 与 catalog query
  迁入 `src-tauri/crates/yss-graph-runtime/`；删除最后的根 `graph` facade、零调用 basis/catalog API、
  未读取且会形成第二事实源的 cached resource catalog，以及生产中恒为空操作的 bind hook；测试故障注入
  只通过 `test-support` feature 暴露，Application 继续唯一拥有 session capture/revalidation/commit。
- [ ] 将 project instance/session/operation/history identity 与 project/resource revision
  迁入 `src-tauri/crates/yss-project-identity/` Pure Leaf；删除根 `project` identity owner 与兼容
  re-export，所有消费方直接依赖新 crate；保留 Project/Graph revision 的显式命名转换，并以
  `test-support` feature 隔离测试专用 revision advancement。
- [ ] 将 project discovery/cleanup 的平台无关进度事件与输出 port 迁入
  `src-tauri/crates/yss-project-progress/` Pure Leaf；Project registry 与 command adapter 直接依赖
  唯一 owner，Tauri 有界队列、Channel 和 wire DTO 留在 Commands，并删除两个零调用的重复事件
  DTO，避免根 facade、幽灵 API 与多事实源回流。
- [ ] 将 persisted computation settings、validation 与 project mutation envelopes 迁入
  `src-tauri/crates/yss-computation-settings/` Pure Leaf；Project/Application/Commands/Event 直接消费
  唯一 owner，manifest 读取严格拒绝非法或未知 settings 字段，并删除 Application 中完全同构的
  mapping error 镜像与无效转换函数。
- [ ] 将 worksheet persisted document、schema version、resource path 与布局常量迁入
  `src-tauri/crates/yss-worksheet-document/` Pure Leaf；Project 仅保留 redirect-safe 扫描、事务 I/O、
  history 与 authority，全部消费者直接依赖唯一 owner；嵌套 encodings 严格拒绝未知字段，并删除根
  resource-path owner、test-only 目录 helper 与生产扩展名字符串副本。
- [ ] 将 project discovery/cleanup 的取消 capability 与 active-task registry 收归
  `src-tauri/crates/yss-project-progress/` Pure Leaf，删除根 `project_picker_task` owner、字符串 sentinel
  与裸 `AtomicBool` 泄漏；使用 typed discovery error 保证递归扫描中途取消仍映射为 `Cancelled`，
  而不是错误漂移为 `ScanFailed`；新任务替换 active task 时先取消旧任务，避免孤儿扫描。
- [ ] 将 project registry 的 canonical record、root identity state 与 persistence port 迁入
  `src-tauri/crates/yss-project-registry-contract/` Pure Leaf，删除根 crate 中字段完全相同的
  `ProjectRegistryRecord` 存储镜像及转换函数；将 registration/root identity 收归
  `yss-project-identity`，避免把持久化 registration ID 错当成 runtime `ProjectInstanceId`，并让
  Project、Application、Commands、Transport 与 SQLite adapter 直接依赖唯一 contract owner；
  重新启用 8 个 application lifecycle 测试，并修复 registry 写入失败时恢复动作从
  `registerDestination` 漂移成 `removeRegistryRecord` 的语义错误。
- [ ] 将 project registration、favorite、cleanup、scan 与路径校验工作流迁入
  `src-tauri/crates/yss-project-registry/` Stateful Project 层；删除根 `project_registry` owner 与兼容
  facade，行为层仅依赖 registry contract、discovery、progress、filesystem identity 等下层 crate，
  SQLite 继续作为 Backend Adapter；删除零调用的 validity wrapper，并以测试侧失败 store 替代生产类型中的
  remove-failure 开关，避免测试后门成为公开 API。
- [ ] 将 `ProjectRegistryStore` 的 SQLx/SQLite 实现迁入
  `src-tauri/crates/yss-project-registry-sqlite/` Backend Adapter 层；删除根 `backend_adapters` owner 与
  兼容 module，Composition Root 直接注入 concrete store；集中 workspace 的 SQLx/Tokio 版本声明，
  并让非法 `is_favorite`/root identity discriminant fail closed，而不是把任意非零整数静默解释为 true。
- [ ] 将 project 根文件名、内容目录、资源扩展名与 index-input 相对路径分类收归
  `src-tauri/crates/yss-project-layout/` 无依赖 Pure Leaf；Graph/Worksheet/Project/Watcher 直接消费
  唯一 owner，删除 `project_io`、registry、graph/worksheet document 与 watcher 中的布局镜像，
  并拒绝空路径、绝对路径及含 parent/root/prefix component 的不安全分类输入；I/O、schema、
  watcher delivery 与 workflow 继续留在各自层。
- [ ] 将 project mutation envelope、resource/document patch、undo/redo transaction 与内存文档状态机迁入
  `src-tauri/crates/yss-project-history/` Project 层；所有消费者直接依赖唯一 owner，删除根
  `project/history.rs` 与兼容 re-export，并移除仅测试可构造、生产永远无法应用的旧 Graph patch 分支；
  filesystem hydration、durable transaction、publication 与 transport mapping 继续留在原有层。
- [ ] 将可取消的 project metadata 递归扫描、跳过目录规则与项目名规范化迁入
  `src-tauri/crates/yss-project-discovery/` Project 层；删除根 `project_scan` owner 与兼容 re-export，
  注册结果继续由 registry workflow 拥有；扫描拒绝进入 Unix symlink 与 Windows reparse point，
  避免越过用户选择的 discovery root 或因目录重定向形成递归循环。
- [ ] 将 `metadata.yssbi` 的当前 schema version、严格 `ProjectManifest` wire 与 computation settings
  fail-closed validation 迁入 `src-tauri/crates/yss-project-manifest/` Pure Leaf；Project I/O 只通过
  唯一受校验构造器生成 manifest，不再拥有重复 schema/validation 定义，validated fields 不公开
  mutation seam；文件系统、运行时 `ProjectData`、时钟与 lifecycle 继续留在 Project 层。
- [ ] 将变量默认值、稳定 `var:{id}` handle 与 tabular literal/snapshot 归一化迁入
  `src-tauri/crates/yss-variable-value/` Pure Leaf；删除 Project 中两个旧 owner 与兼容入口，复合类型
  默认值不得伪造示例数据，canonical handle 必须对应 snapshot，DataSeries 归一化必须保留附加 metadata；
  Project 继续拥有状态/事务/激活，并在非法持久化变量上 fail closed 而不是静默吞错。
- [ ] 将用户可见路径的 Windows extended-length prefix 移除语义迁入无依赖
  `src-tauri/crates/yss-path-display/` Pure Leaf；Project registry 与 Application query 直接消费唯一
  owner，删除根 `project/path_format.rs` 与兼容 capability，并让 `\\?\` / `\\?\UNC\` 处理在所有
  宿主平台保持一致；路径存在性、canonicalization、校验与 I/O 继续由调用方拥有。
- [ ] 将内存 `ProjectData`、`ProjectMetadata` 与 Graph resource aggregate 迁入
  `src-tauri/crates/yss-project-model/` Project 层；删除根 `project_data.rs`、`project_metadata.rs`
  与兼容 re-export，移除零调用的整包 JSON/`info`/隐式 metadata 刷新 API，统一复用
  `yss-graph-document::GraphResourceKind`；Project lifecycle 显式拥有 export-time 时钟，磁盘 wire
  继续由 manifest、Graph/Worksheet contract 与 Project I/O 分别拥有。
- [ ] 将函数文档到强类型 editor pin/projection 的转换迁入
  `src-tauri/crates/yss-function-editor-projection/` Project 层；以 `ResourceRevision` 替代内部裸
  `u64`，让 Project index 与 mutation event 复用同一个严格 camelCase wire，并删除根
  `function_editor_projection.rs`、三处 parameter/signature 展开逻辑及 Transport 同构 DTO 镜像。
- [ ] 将安全 project-relative path、文件 change kind、显式重扫请求与 index invalidation 结果迁入
  `src-tauri/crates/yss-project-change/` Pure Leaf；删除根 `project_change.rs` contract owner，避免将
  watcher 故障伪造为 metadata 文件变更，并将无关路径从 error control flow 改为 no-op；notify adapter
  忽略普通 read/open access 但保留 `Close(Write)`，同时保留 rename 语义及跨边界事件中的安全根内路径；
  ProjectState 重读协调继续留在 Project 层。

## 2026.08.31

- [ ] 将运行期 project aggregate 的原子候选 patch 迁入 `yss-project-model::ProjectDataPatch`；删除根
  `project/resource_patch.rs` 与 facade，并与 `yss-project-history::ResourceDocumentPatch` 的持久化历史
  payload 明确分名，避免两个不同语义继续共享 `ResourceDocumentPatch` 名称；ProjectState 保留锁、事务、
  I/O、history 转换与 publication authority。
- [ ] 将 project/session 绑定的 operation admission、防重放集合与 RAII reservation 状态机迁入
  `src-tauri/crates/yss-project-operation/` Stateful Project 层；删除根 `resource_mutations/operation_ledger.rs`
  与兼容 re-export，并以 canonical `ProjectSessionId` 替代 ledger 自建 UUID epoch，消除第二会话事实源；
  ProjectState 继续拥有 publication 线性化、跨状态锁顺序与现有 filesystem error 分类映射。
- [ ] 将 project instance/resource 绑定的 lifecycle token admission、load/unload/rename 排他规则、
  predecessor chain 与 RAII guard 状态机迁入 `src-tauri/crates/yss-resource-lifecycle/` Stateful Project 层；
  删除根 `project/resource_lifecycle.rs` owner 与兼容 facade，核心 API 仅依赖 canonical `ProjectInstanceId`；
  恢复原先被永久关闭的 17 个状态机测试，并删除零调用的 panic getter/测试探针；跨状态 session 校验、
  activation publication 锁顺序与 filesystem error 分类映射继续留在根 Project 层。
- [ ] 将 native project-root identity、入口路径 binding/revalidation、root lease/lifecycle admission、原子
  transaction/rollback 与 recovery marker 迁入 `src-tauri/crates/yss-project-filesystem/` Stateful Project 层；
  删除根 `project/filesystem/` owner、`ProjectFilesystemError` 镜像和兼容 facade，并把 transaction context
  收窄为 root、operation id 与 recovery marker，避免 filesystem 反向依赖完整 `ProjectSession`；ProjectState
  publication、resource revision 校验与 document serialization 继续留在根 Project 层；错误码和 recovery
  分类仅通过 Application-owned failure view 进入 Commands，通用 Transport error 不依赖 Project/Application。
- [ ] 将 Excel workbook sheet inspection 与 Sheet→CSV bridge 迁入现有 `yss-tabular-io` Database Core；
  删除根 `database/excel_reader` owner/facade，将 `calamine` 依赖下沉到唯一 owner，并让 Application 与
  DuckDB reader 直接调用 typed `ExcelIoError` API，为 `duckdb_reader` 整体迁入 `yss-duckdb` 解除反向依赖。
- [ ] 将根 `database/duckdb_reader.rs` 的 table lifecycle、ingest、Arrow bridge、catalog/display metadata 与
  paged/column query 整体迁入 `yss-duckdb::table`；删除根 owner/re-export，所有 Application、Project、Database
  和 integration test 消费方直接依赖 engine crate，并将不再属于根 package 的 `polars-arrow` 依赖一并下沉；
  同时统一 DuckDB 父目录创建，并修复删除/覆盖表时遗留命名 ENUM type、无 metadata table 时先删表后报错的问题。
- [ ] 将共享 `EditOperation`、`EditHistory` 与 `EditState` 迁入无 Polars/DuckDB 依赖的
  `src-tauri/crates/yss-database-edit/` Database Core；删除根 `database/edit_operation.rs` owner/facade，并将
  DataFrame apply/reverse/cast 迁入现有 `yss-tabular-polars::edit` adapter，使 DuckDB 与 Loaded state 复用同一
  operation model，同时下沉 root package 不再直接使用的 `polars-dtype` 依赖，并删除零调用的
  `EditOperation` serde wire compatibility，只保留实际跨 IPC 的 `EditState` camelCase projection。
- [ ] 将根 `database/duckdb_editing.rs` 与 `duckdb_column_snapshot.rs` 整体迁入现有 `yss-duckdb` engine
  crate；让根 `DatabaseInstance` 仅调用 typed operation API，不再自行打开 DuckDB edit connection，并使
  cell/add-row/delete-rows/apply/reverse 全部在 engine transaction 中完成；同时修复未排序 `indices` 与
  `rowIds` 配对漂移、多行删除中途失败产生半提交、add-row reverse 已命中 null row 时仍提前求值 index
  fallback，以及 update 未命中 rowid 却静默成功的问题。
- [ ] 修复 DuckDB cell edit 将大于 `i64::MAX` 的 JSON unsigned integer 经 `f64` 中转而丢失精度的问题；
  SQL numeric literal 直接使用 `serde_json::Number` 的规范十进制表示，并用 `u64::MAX -> UBIGINT` 回归测试
  锁定无损语义。
- [ ] 将根 `database/sql_reader.rs` 与 `sqlite_reader.rs` 合并迁入职责命名的 `yss-sql-source` Database Core；
  统一 SQLite/PostgreSQL/MySQL table discovery、identifier quoting、连接配置、SQLx decode 与 Polars
  materialization，删除根 facade 以及 root production 的 Tokio/SQLx 依赖，并由 Application 直接调用 typed API。
- [ ] 修复 external SQL import 将 PostgreSQL `INT2/INT4`、MySQL 各宽度 signed/unsigned integer 等合法值静默
  降级为 null、BLOB 被替换成 `<N bytes>` 字符串、空表丢失列 schema、list-table decode error 被 `filter_map`
  吞掉、SQLite 路径手拼 URL，以及 `ssl`/`charset`/`auto_create` persisted config 未生效的问题；unsupported
  source type 改为 fail closed，并验证同步 API 在既有 Tokio runtime 内不会 nested-`block_on` panic。
- [ ] 将根 `database/schema_snapshot.rs` 的 runtime schema facts、revision projection 与 Polars/DuckDB
  physical metadata normalization 迁入职责命名的 `src-tauri/crates/yss-database-schema/` Database Core；删除
  根 owner/facade，并让 Application、Database runtime 与 Transport conversion 直接消费唯一 schema owner，
  session/runtime authority 继续留在根 Database 层。
- [ ] 修复 DuckDB `TIMESTAMP` metadata 经 `duckdb_type_to_raw_string` 产生精确 `DateTime` 后，在 schema
  normalization 中因仅识别 `DateTime(...)` 而静默降级为 `DataType::Any` 的漂移；同时锁定 Polars temporal
  dtype、nullable、revision 与非法列名 fail-closed 语义。
- [ ] 将剩余 session-scoped database instance/state、declaration observation/revision authority、
  admission/drain/recovery、physical routing 与 typed query/edit handoff 整体迁入职责命名的
  `src-tauri/crates/yss-database-runtime/` Database Core；删除根 `src/database/` owner/facade，所有
  Application、Commands、Transport 与 integration test 消费方直接依赖新 crate，且不创建混合领域的
  catch-all `yss-backend`。
- [ ] 清理 database runtime 抽取后的边界漂移：公开最小的不透明 session/transaction API，保留 registry
  record 与 physical state 私有；删除可绕过真实 runtime 伪造 catalog snapshot 的 test-only minting seam、
  永远不可产生的内部 driver compensation 分支及重复 restore wrapper，并以架构测试锁定 runtime 不反向依赖
  Project、Application、Commands、Transport 或 Tauri。
- [ ] 将平台中立的 project watcher epoch、delivery admission、factory/session/drain protocol、超时 ownership
  与 replacement 状态机迁入 `src-tauri/crates/yss-project-watcher/` Application service；根 Application 只保留
  Project authority reconciliation，Notify 适配器直接消费 crate contract，并删除未使用的重复 quiet-period 常量
  与生产不可构造的 `DeliveryFailed` 终态。
- [ ] 将根 `platform/project_file_watcher` 的 native observation、notify event 映射、bounded debounce、worker lifetime
  与 drain completion 整体迁入 `src-tauri/crates/yss-project-watcher-notify/` Platform Adapter；删除空根
  `platform` facade 与 root package 的直接 `notify` 依赖，让 composition root 只负责 adapter 注入。
- [ ] 将系统 Julia executable discovery、版本校验、Windows Juliaup 安装与 background command policy 迁入
  `src-tauri/crates/yss-julia-runtime/` Backend Adapter；Commands/worker 直接消费 typed runtime API，删除根 facade，
  并以 `JuliaRuntimeError` 替代公开 `String` error，同时保留无 stderr/stdout 命令失败的 process status 诊断。
- [ ] 将根 Julia reusable process、embedded assets、JSON-RPC、progress/cancel/restart、typed worker error 与
  app-owned task-directory lifecycle 迁入 `src-tauri/crates/yss-julia-worker/` Backend Adapter；SCI/Bayes artifact
  trait 适配留在 adapter 一侧，删除根 worker facade、公开 `String` compatibility API 与状态机不可达分支，并让
  Commands、composition root 及 Bayes adapter 直接消费唯一 worker owner。
- [ ] 将 backend-neutral statistical input/settings、执行控制、取消 capability 与稳定 SCI error code 迁入
  `src-tauri/crates/yss-sci-contract/` Pure Leaf；删除根 `sci/api/computation.rs`、`sci/api/control.rs`、
  `sci/error.rs` facade 和零生产调用的 Application `DataValue` 映射，让 workflows、SCI runtime、Execution
  adapter 与 Julia Bayes adapter 直接消费唯一 owner，并以 `StatisticalInput::try_new` 拒绝空白名称和非有限数值。
- [ ] 将 Bayes draft、expression parser、structured validation 与 validated immutable spec 构造迁入
  `src-tauri/crates/yss-bayes-model/` Pure Leaf；删除根 `sci/api/bayes` model facade，让 Commands、Application、
  worker validation、integration tests 与 Julia adapters 直接消费唯一 owner；同时以内部 parts object 收敛
  多参数构造，统一 conversion/worker 的 spec validator，并消除 `ValidationReport.ok`/errors 双事实源、
  非有限 prior 漏检及 draft-to-spec 的生产 `expect` 分支。
- [ ] 将 Bayes diagnostics、task/result projection、artifact manifest 与 plot/page DTO 迁入
  `src-tauri/crates/yss-bayes-result/` Pure Leaf；删除根 `sci/api/bayes` result/contract facade、旧 exchange contract
  与仅测试挂载的重复 Julia Bayes backend，让 Commands、Application、worker 和 Julia adapter 直接消费唯一 owner；
  artifact lease 由 Application 的 materialization 清单唯一管理，不再嵌入可序列化 result，同时拒绝把 PNG/任意 binary
  伪装成 Arrow IPC。
- [ ] 将 backend-neutral Bayes validated task、opaque generation/artifact handle、terminal/error contract 与 worker port
  迁入 `src-tauri/crates/yss-bayes-worker/` Pure Leaf；删除根 `sci/api/bayes/worker` owner/facade，让 Application 与
  Julia adapter 直接消费唯一 crate，并以不可构造的 `BayesWorkerAuthority` 和 `BayesWorkerClient` 临时借用收敛
  handle/result 铸造权限，避免因跨 crate 迁移而把旧 `pub(crate)` 构造器直接公开；Julia process 与 task-directory
  lifecycle 继续仅由 `yss-julia-worker` 拥有。
- [ ] 删除根 `sci/api/bayes` 中仅测试启用的 `BayesBackend` 与 Polars input-validation 第二路径；让
  `BayesInferenceService` 强制持有非可选 `BayesWorkerClient` 和 app-data root，生产与测试共同通过
  `BayesWorkerPort` 执行；删除双 queue/runner、空 SCI facade、不可达 cancel backend 分支与零调用 artifact reader，
  并由架构测试禁止恢复 test-only backend、可选 worker 或 Transport validation 镜像。
- [ ] 将根 `julia/bayes_worker_adapter` 的 Julia model kernel 生成、typed exchange files、worker task 状态、
  cancel/result/artifact port 实现整体迁入 `src-tauri/crates/yss-bayes-worker-julia/` Backend Adapter；删除空根
  `julia` facade，让 composition root 直接构造具体 adapter，并禁止新 crate 反向依赖 Tauri、Application、
  Commands、Project 或 Database。
- [ ] 将剩余根 `src/sci` 同步 runtime API、regression/panel models 与 Rust adapters 整体迁入职责命名的
  `src-tauri/crates/yss-sci-runtime/` SCI Core；删除根 facade，让 Application、Execution adapter 与 Commands
  直接消费唯一 owner，并把 time-series golden tests/fixtures 随 owner 下沉；root package 不再直接依赖
  `yss-sci`、`rand` 或 `statrs`，runtime 不反向依赖 Tauri、Project、Database、Execution 或 Julia。
- [ ] 清理 SCI runtime 迁移后的失效分支与重复事实：Panel DID 使用 typed `DidFakeGroupError`、一次外层结构校验
  与唯一实体 ID 集合，避免稀疏 ID 生成 phantom entities；regression report 改为一次 typed serialization 并将
  失败映射为 `SciError`，不再依赖对象形状 `expect` 或多阶段可变 JSON；以 crate ownership/semantic guards
  禁止根 SCI、重复验证和旧 consumer route 回流。
- [ ] 将根 `src/project/` 的 `ProjectState`、session/instance authority、resource revision、history hydration、
  持久化事务编排与 publication 整体迁入 `src-tauri/crates/yss-project/` Stateful Project 层；删除根 owner/facade、
  重复 resource mutation 路径、永久关闭的边缘测试与零调用 hooks，让 Application/Commands/Composition Root 直接
  依赖唯一 crate；同时修复 Graph move undo/redo 误入恒定失败 legacy rename stub，并以可构造磁盘计划回归锁定。
- [ ] 将根 `src/application/` 的跨 Project、Execution、Database、Graph、SCI 与 Bayes 用例编排整体迁入
  `src-tauri/crates/yss-application/`；删除根 owner/facade，让 Commands 与 composition root 直接消费唯一
  Application crate，并以最小公开 typed API、`test-support` feature 和架构门禁禁止 Tauri、Commands、Schema
  或根包反向依赖。
- [ ] 清理 Application 抽取后的重复与失效事实：数据库导出失败统一进入 typed cleanup、保留 session transition
  主错误并记录 cleanup 次错误；删除零调用 database mutation/session hooks、重复 session/database receipt 字段、
  不可构造 result-state/data-series wire 分支与永久关闭测试遗留的发射器，同时移除根包不再直接消费的依赖。
- [ ] 收窄 `yss-application` 跨 crate API：以 `HypothesisApplicationError` 替代 hypothesis/`at()` 的公开
  `String` error，隐藏仅内部使用的解析与 candidate-install 类型，并新增 ownership 门禁禁止根 Application facade、
  Tauri/Transport 反向依赖及错误层分类回流。
- [ ] 将 Bayes artifact reader port/error 从 Application 抽取到
  `src-tauri/crates/yss-bayes-artifact-contract/` Pure Leaf；Application 与 concrete reader 直接消费唯一 contract，
  禁止 contract 反向依赖 Polars、Tauri、Application 或具体 filesystem I/O。
- [ ] 将根 Bayes/Polars artifact reader 迁入 `src-tauri/crates/yss-bayes-artifact-polars/` Backend Adapter，
  删除 Application 内复制的 DataFrame 解析与测试镜像并下沉测试；malformed/null/non-finite rows、空 predictive
  artifact 和零 plot budget 必须 fail closed，禁止静默丢行、除零及 Tauri/Application 回边。
- [ ] 将根 Execution→SCI adapter 迁入 `src-tauri/crates/yss-execution-sci-adapter/`，仅保留生产实际消费的
  ACF/PACF typed port 并让零 lag fail closed；删除零生产调用的 regression/KDE 与 relational port families、
  永远 unavailable 的 relational stub、透传 session ID 的 resource factory callback 及整个根 `backend_adapters` facade。
- [ ] 将根 `commands`、`schema`、`error` 与 `event` 整体迁入 `src-tauri/crates/yss-api/`，由私有 transport
  modules 唯一拥有 command handlers、wire mapping、稳定错误与 event/channel delivery，只公开 canonical
  `invoke_handler` 给 composition root；删除永久关闭的兼容 helpers/tests、不可构造 DTO 分支与未使用依赖，
  并禁止根 transport facade、重复 command registry 或 composition-only adapter 依赖回流。
- [ ] 清理 `yss-api` project progress transport 的线程启动失败路径：以 typed spawn error 替代生产 `expect`，
  在 command 边界集中映射稳定错误码与 incident ID，并确保 worker 创建失败时不会登记残留的活动取消任务；
  同时移除当前 wire 文档中的过时 legacy 表述。
