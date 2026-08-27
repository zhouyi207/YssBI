# 在正式打包分发前，可以等渲染完毕再显示窗口
# 在目前的开发环境中，不要这样做，可以取消 debug
# 项目未发布 不做任何迁移处理

每次更新版本都需要

由于历史代码重构原因，目前项目中存在许多的历史遗留代码，或多余或逻辑重复或实现低效；请检查整体项目，寻找出项目中的重复逻辑和未使用的逻辑，分析必要性，如果有更高效的更干净的架构请添加到 todo 的 v1.0 待办中，如果单纯的逻辑重复或者多余，也请添加到 v1.0 待办中

请分析这个问题有没有必要修复，如果有必要，则使用高效且干净的架构来执行这个逻辑，同时清除掉无效逻辑代码和重复逻辑代码

重复逻辑问题？无效逻辑问题？代码漂移问题？多事实源问题？代码冲突问题？无效函数问题？deprecated 兼容问题？

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
- [ ] **CI 扩展：`cargo clippy` + integration tests 矩阵**：在 `cargo test` 之外增加 `cargo clippy --all-targets`（先 `yss-sci` 修 error 再全 workspace）；与前端 `typecheck` 并列，形成全栈静态门禁。
- [ ] **CI 门禁 `tsc --noEmit`**：`package.json` 增加 `typecheck` script，CI 与 pre-push 跑 `pnpm typecheck`（`noUnusedLocals` 已开，需防止类型债再次累积）。
- [ ] **CI 门禁：`typecheck` + vitest + `cargo test` 并列**：`tsc` 无法捕获仅运行时才暴露的 API 形参错误（如 `batchCreateNodes` 三参数旧调用）；`package.json` scripts 与 CI workflow 至少跑 `tsc --noEmit`、核心 vitest 套件、Rust integration tests。
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

deserializeGraph 这个玩意是干嘛的，好多地方都没必要用他，感觉好卡

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


## 项目分析

我看了你这个仓库当前的 `shadcn` 分支，包括 README、前后端目录、Cargo 依赖、services、features、commands、execution、TODO 和 CI。整体上我认为 **方向是对的，而且已经明显超过普通 Tauri CRUD 项目**：你实际上在做一个「节点式统计分析 IDE / 桌面 BI 工作台」。README 里已经覆盖数据管理、节点工作流、计量经济学、时间序列、因果推断和可视化。([GitHub][1])

但现在确实到了一个很关键的阶段：**接下来最应该做的是收敛架构，而不是继续堆功能。**

我会把优化优先级排成下面这样。

### P0：先把后端真正拆成“宿主层”和“计算层”

你现在已经有：

```text
src-tauri/
├── src/
│   ├── application
│   ├── commands
│   ├── database
│   ├── execution
│   ├── graph
│   ├── node_system
│   ├── project
│   ├── sci
│   └── ...
│
└── sci/
    └── 独立 yss-sci crate
```

这说明你已经开始做正确的拆分。([GitHub][2])

但是主 `yssbi` crate 依然直接依赖：

```text
ndarray
polars
polars-arrow
polars-dtype
statrs
sqlx
duckdb
calamine
rand
...
```

而 `yss-sci` 又已经存在。([GitHub][3])

我建议继续收敛成：

```text
Tauri / Host
        │
        ▼
Application
        │
        ▼
Execution Engine
        │
        ├───────────────┐
        ▼               ▼
Data Engine         Compute Engine
Polars/DuckDB       Rust / Julia
                        │
                        ▼
                     yss-sci
```

也就是说：

```text
yssbi
负责：
Tauri
窗口
IPC
项目
Graph
Executor
数据生命周期
任务调度

yss-sci
负责：
OLS
GLS
WLS
IV
Panel
VAR
VECM
DID
Hypothesis Test
统计量
线性代数
```

尤其应该避免：

```rust
commands -> 统计实现
```

而变成：

```text
command
   ↓
application/use-case
   ↓
execution
   ↓
compute backend
   ↓
yss-sci / Julia
```

你自己的 TODO 已经提到“Rust 保留宿主、数据层和必要 fallback；科学计算逐步迁移 Julia”，这个总体思想是合理的。([GitHub][4])

---

## P0：把 Execution Engine 做成整个 YssBI 的核心

我认为 **YssBI 最值钱的代码将来可能不是 OLS，也不是 UI，而是 execution engine。**

README 已经显示你的核心交互是：

```text
Node
 ↓
Pin
 ↓
Connection
 ↓
Graph
 ↓
Execution
```

并且你已经有独立：

```text
execution/
graph/
node_system/
```

目录。([GitHub][2])

建议进一步明确一个非常重要的边界：

```text
Graph ≠ Execution
```

Graph 只描述：

```text
节点是什么
连接是什么
参数是什么
依赖是什么
```

Execution 才负责：

```text
拓扑排序
dependency resolution
dirty propagation
cache
task scheduling
cancellation
progress
error propagation
parallel execution
```

最终可以形成：

```rust
ExecutionPlan
    ↓
TaskGraph
    ↓
Scheduler
    ↓
Executor
    ↓
Backend
```

类似：

```rust
trait ExecutionBackend {
    async fn execute(
        &self,
        task: &ExecutionTask,
        ctx: &ExecutionContext,
    ) -> Result<TaskOutput, ExecutionError>;
}
```

backend 可以有：

```text
PolarsBackend
DuckDbBackend
RustSciBackend
JuliaBackend
AIBackend
```

这样以后加 Python、R、GPU 都不会重新设计节点系统。

---

# P0：解决“计算导致 UI 卡死”

你 TODO 里自己已经发现了：

> 按下按钮涉及大量计算的时候，页面会卡死。([GitHub][5])

这个千万不要简单理解成：

> `spawn 一个 thread`

真正应该做的是 **Task System**。

例如：

```text
ExecutionTask
├── id
├── node_id
├── state
│   ├── queued
│   ├── running
│   ├── completed
│   ├── failed
│   └── cancelled
├── progress
├── cancellation_token
└── result
```

前端：

```text
Run Node
   ↓
invoke start_execution
   ↓
立即返回 task_id
   ↓
Rust 后台执行
   ↓
event:
execution:started
execution:progress
execution:completed
execution:failed
```

而不是：

```text
React
 ↓
invoke()
 ↓
等 20 秒
 ↓
Result
```

这对：

```text
VAR
VECM
Bayes
大数据聚合
数据库 import
未来 AI
```

全部有用。

以后还可以自然支持：

```text
Cancel
Retry
Pause
Parallel
Queue
Execution history
```

---

# P1：你现在的前端目录有一点“重复架构”

目前前端同时存在：

```text
src/
├── app
├── components
├── features
├── lib
├── services
├── shared
├── utils
├── views
```

而 `features` 内部又已经定义：

```text
core
domain
application
```

并且明确规定依赖关系。([GitHub][4])

这个思想本身很好。

问题是：

```text
services/
features/
views/
components/
shared/
lib/
utils/
```

长期非常容易产生归属不明确：

> “这个函数到底放 services、shared、utils 还是 feature？”

你现在 `services` 已经包含：

```text
bayes
clipboard
database
graph
ipc
julia
log
nodeSystem
project
result
stats
variable
window
worksheet
```

([GitHub][6])

这里已经有一点明显的“横向 service 大目录”趋势。

我更推荐：

```text
src/
├── app/
│
├── features/
│   ├── graph/
│   ├── project/
│   ├── dataframe/
│   ├── statistics/
│   ├── worksheet/
│   ├── visualization/
│   └── workbench/
│
├── platform/
│   ├── tauri/
│   ├── ipc/
│   └── window/
│
└── shared/
    ├── ui/
    ├── hooks/
    ├── types/
    └── utils/
```

然后例如：

```text
features/project
├── api
├── model
├── store
├── ui
└── lib
```

这样：

```text
ProjectService
ProjectStore
ProjectView
ProjectDTO
```

全部围绕 `project` 放置。

这比现在：

```text
services/project
features/...
views/...
```

更适合一个越来越大的应用。

---

# P1：你现在正在做的 AppError 非常值得完成

TODO 里这一条，我非常赞同：

> 绝大多数 `#[tauri::command]` 仍然是 `Result<_, String>`，准备统一成结构化 `AppError`。([GitHub][5])

应该尽快做完。

不要：

```rust
Result<T, String>
```

而应该：

```rust
struct AppError {
    code: ErrorCode,
    message: String,
    details: Option<Value>,
}
```

例如：

```json
{
  "code": "DATAFRAME_COLUMN_NOT_FOUND",
  "message": "Column `age` does not exist",
  "details": {
    "column": "age"
  }
}
```

前端：

```ts
switch (error.code) {
  case "PROJECT_NOT_FOUND":
  case "NODE_EXECUTION_FAILED":
  case "DATABASE_CONNECTION_FAILED":
}
```

这件事情收益非常高。

因为以后 AI Agent 调用 YssBI 工具时，也可以直接理解：

```text
code
message
details
```

而不是解析字符串。

---

# P1：DTO 自动生成，我建议直接做

TODO 里你也已经意识到了：

> Rust DTO 和 TypeScript 手写 types.ts 容易漂移，考虑 typeshare / ts-rs。([GitHub][5])

我的答案是：

**做。**

你这个项目非常适合。

因为数据类型本身已经很多：

```text
GraphInstanceDTO
DatabaseDecl
DatabaseEngine
DataType
Pin
Node
Variable
ExecutionResult
RegressionResult
```

手动：

```text
Rust struct
+
TypeScript interface
```

迟早出错。

可以变成：

```rust
#[derive(Serialize, Deserialize, TS)]
#[ts(export)]
pub struct RegressionResult {
    ...
}
```

然后：

```text
cargo xtask bindings
```

自动产生：

```text
src/generated/api/
```

甚至再进一步：

```text
Rust Command
       ↓
generated TS binding
       ↓
typed invoke()
```

最终让：

```ts
invoke<any>()
```

基本消失。

---

# P1：数据库层需要选一个明确主战略

你现在同时有：

```text
Polars
DuckDB
SQLx
Arrow
Excel
```

Cargo 已经明确体现出来。([GitHub][3])

这不是坏事。

但必须定义谁干什么。

我推荐非常明确地规定：

```text
DuckDB
= 项目内持久化 + SQL analytics

Polars
= DataFrame / LazyFrame transformation

Arrow
= 数据交换格式

SQLx
= 外部数据库连接

yss-sci / Julia
= Statistics
```

不要让：

```text
DuckDB
Polars
Julia DataFrame
Rust Vec
JSON
```

互相随意转换。

理想数据通道应该是：

```text
External DB
    ↓
Arrow
    ↓
Polars / DuckDB
    ↓
Arrow
    ↓
Statistics
```

也就是把 **Arrow 当成 YssBI 的数据 ABI**。

这会让你以后接：

```text
Julia
Python
GPU
Remote Executor
```

都简单很多。

---

# P1：不要把 DataFrame 本体放 Zustand

你已经使用 Zustand。package.json 里目前是 Zustand 5。([GitHub][7])

Zustand 很适合：

```text
activeProjectId
selectedNodeId
openedTabs
layout
theme
selection
panel state
```

但是不应该存：

```text
1,000,000 rows DataFrame
```

或者：

```text
大型 RegressionResult 原始数据
```

应该遵循：

```text
Frontend = references
Backend = actual data
```

例如：

```ts
{
  dataframeId: "df_123",
  rowCount: 12_000_000,
  schema: [...]
}
```

真正的数据：

```text
Rust / DuckDB / Polars
```

前端 DataGrid 请求：

```text
rows 1000..1100
```

你 README 已经明确说 DataView 面向大数据量优化并用了虚拟化，这个方向是正确的。([GitHub][1])

---

# P1：测试现在有点过度“Architecture Contract 化”

我注意到已经存在：

```text
architecture.test.ts
observabilityArchitectureContract.test.ts
userFeedbackArchitectureContract.test.ts
```

同时 Rust commands 目录里面还有：

```text
command_*_tests.rs
command_blueprint_graph_phase1_tests.rs
command_node_system_reroute_tests.rs
...
```

([GitHub][8])

这里我建议稍微控制。

特别是 AI 辅助开发很容易生成：

```text
ArchitectureContractTest
RegressionContractTest
ModuleBoundaryTest
...
```

最后导致：

> 改文件路径 → 一堆测试挂了

而不是：

> 行为错了 → 测试挂了

我建议测试比例更接近：

```text
60% domain / calculation tests
25% integration tests
10% regression tests
5% architecture tests
```

对于统计软件尤其应该重视：

```text
Golden Tests
```

例如：

```text
OLS:
YssBI
vs
Stata
vs
R
```

验证：

```text
coef
std error
t
p
R²
F
CI
```

而不是大量测试：

```text
某文件必须 import 某路径
```

你已经计划 Rust/Julia golden fixture，这其实是非常好的方向。([GitHub][4])

---

# P1：CI 需要比现在再强一点

当前 `.github/workflows` 里只有：

```text
publish.yml
```

([GitHub][9])

而 package.json 已经有很完整的：

```text
verify
verify:frontend
verify:rust
verify:full
```

([GitHub][7])

所以非常适合增加：

```text
ci.yml
```

PR 至少跑：

```text
pnpm install --frozen-lockfile
pnpm typecheck
pnpm test

cargo fmt --check
cargo check
cargo clippy
cargo test -p yss-sci
```

其中建议加上的关键一项是：

```text
cargo clippy
```

目前你的 `verify:rust` 只有：

```text
fmt
check
```

没有 clippy。([GitHub][7])

---

# P2：Cargo 编译时间可以进一步优化

你后端现在依赖很重：

```text
Polars
DuckDB bundled
SQLx
ndarray
faer
statrs
Tauri
```

([GitHub][3])

这个组合在 Windows 上编译会非常重。

而你现在：

```text
cargo test --jobs 1
```

已经说明你可能是在主动控制内存。([GitHub][7])

建议逐渐拆 workspace：

```text
crates/
├── yss-core
├── yss-graph
├── yss-execution
├── yss-data
├── yss-sci
└── yss-app
```

不一定现在一次性拆完。

第一步我只建议：

```text
yss-sci
yss-data
yss-execution
```

三个真正稳定的边界。

好处是：

```text
改 React/Tauri
不会重新编整个 science stack

改 OLS
不用影响 graph

改数据库
不用重新编统计模型
```

---

# P2：README 对外展示还有非常大的提升空间

这个项目技术内容其实比 README 表现出来的成熟很多。

目前 README 相对短，而且还有：

```text
数据采集？
报告输出？
```

这样的问号。([GitHub][1])

如果准备开放给别人用，我建议 README 首页直接变成：

```text
YssBI
Visual Statistical Computing Environment

[Screenshot]

Why YssBI
• Visual workflow
• Econometrics
• Large datasets
• Native desktop
• Reproducible analysis

Architecture

Node → Graph → Execution Engine → Data/Compute Backend

Features

Screenshots

Roadmap

Contributing
```

尤其是你已经有非常多：

```text
OLS
WLS
GLS
2SLS
LIML
FE
RE
VAR
VEC
DID
Diagnostics
```

([GitHub][1])

对一个 2-star 项目来说，**代码能力明显比项目包装强。**

---

# 我认为目前架构最大的潜在风险

不是性能。

也不是 React。

也不是 Rust。

而是：

```text
Node System
Graph
Execution
Data
Statistics
Project
UI State
```

逐渐互相知道太多东西。

你需要强制变成：

```text
                  ┌──────────────┐
                  │      UI      │
                  └──────┬───────┘
                         │
                  Application
                         │
           ┌─────────────┴────────────┐
           │                          │
        Graph                     Project
           │
           ▼
    Execution Engine
           │
      ┌────┼────┬─────────┐
      ▼    ▼    ▼         ▼
   Polars DuckDB RustSci Julia
```

其中非常关键：

```text
Graph 不知道 UI
Graph 不知道 Tauri
Sci 不知道 Graph
Sci 不知道 Tauri
DuckDB 不知道 React
Execution 不知道具体 UI
```

这就是我认为 YssBI 接下来最值得守住的东西。

---

## 如果让我给当前项目打一个“下一阶段优先级”

| 优先级    | 工作                            | 收益    |
| ------ | ----------------------------- | ----- |
| **P0** | Execution Task System         | ★★★★★ |
| **P0** | 锁定 Graph / Execution / Sci 边界 | ★★★★★ |
| **P0** | 长计算后台化 + cancel/progress      | ★★★★★ |
| **P1** | 统一 AppError                   | ★★★★★ |
| **P1** | Rust → TS DTO 自动生成            | ★★★★★ |
| **P1** | Arrow 作为数据交换 ABI              | ★★★★★ |
| **P1** | 收敛前端 features/services 结构     | ★★★★☆ |
| **P1** | Golden statistical tests      | ★★★★★ |
| **P1** | CI + clippy                   | ★★★★☆ |
| **P2** | workspace 进一步拆 crate          | ★★★☆☆ |
| **P2** | README / docs 重构              | ★★★★☆ |
| **P3** | 再增加新的统计模型                     | ★★☆☆☆ |

所以我反而建议你 **暂时少加几个统计节点**。

你现在已经有足够多功能证明项目方向了。([GitHub][1])

接下来应该花一轮开发周期，把：

**Graph → ExecutionPlan → Task → Backend → Result**

这一条链做成整个 YssBI 的“脊柱”。

如果这一层设计得好，之后无论你加 **LLM Agent、Julia、Python、GPU、远程计算、多核并行**，都只是增加新的 Backend，而不用重新推翻 YssBI。

[1]: https://github.com/zhouyi207/YssBI "GitHub - zhouyi207/YssBI: YssBI 是一个基于 Blueprint 的桌面数据分析与可视化应用 · GitHub"
[2]: https://github.com/zhouyi207/YssBI/tree/shadcn/src-tauri/src "YssBI/src-tauri/src at shadcn · zhouyi207/YssBI · GitHub"
[3]: https://github.com/zhouyi207/YssBI/blob/shadcn/src-tauri/Cargo.toml "YssBI/src-tauri/Cargo.toml at shadcn · zhouyi207/YssBI · GitHub"
[4]: https://github.com/zhouyi207/YssBI/tree/shadcn/src "YssBI/src at shadcn · zhouyi207/YssBI · GitHub"
[5]: https://github.com/zhouyi207/YssBI/blob/shadcn/TODO.md "YssBI/TODO.md at shadcn · zhouyi207/YssBI · GitHub"
[6]: https://github.com/zhouyi207/YssBI/tree/shadcn/src/services "YssBI/src/services at shadcn · zhouyi207/YssBI · GitHub"
[7]: https://github.com/zhouyi207/YssBI/blob/shadcn/package.json "YssBI/package.json at shadcn · zhouyi207/YssBI · GitHub"
[8]: https://github.com/zhouyi207/YssBI/tree/shadcn/src/features "YssBI/src/features at shadcn · zhouyi207/YssBI · GitHub"
[9]: https://github.com/zhouyi207/YssBI/tree/shadcn/.github/workflows "YssBI/.github/workflows at shadcn · zhouyi207/YssBI · GitHub"


这种代码是不是没有什么必要


+pub fn canonical_port_title(key: &str) -> Box<str> {
    132 +    let title = match key {
    133 +        "value" => "Value",
    134 +        "left" => "Left",
    135 +        "right" => "Right",
    136 +        "result" => "Result",
    137 +        "input" => "Input",
    138 +        "output" => "Output",
    139 +        "enter" => "Enter",
    140 +        "then" => "Then",
    141 +        "true" => "True",
    142 +        "false" => "False",
    143 +        "condition" => "Condition",
    144 +        "operands" => "Operands",
    145 +        "source" => "Source",
    146 +        "dataframe" => "DataFrame",
    147 +        "series" => "Data Series",
    148 +        "values" => "Values",
    149 +        "samples" => "Samples",
    150 +        "sample_count" => "Sample Count",
    151 +        "maximum_lag" => "Maximum Lag",
    152 +        "standard_deviation" => "Standard Deviation",
    153 +        "lower_bound" => "Lower Bound",
    154 +        "upper_bound" => "Upper Bound",
    155 +        "then_source" => "Then Source",
    156 +        "else_source" => "Else Source",
    157 +        "initial_source" => "Initial Source",
    158 +        "next_source" => "Next Source",
    169 +    };
    170 +    title.into()
    171 +}

## 2026.08.27

- [ ] 在 `docs/superpowers/plans` 新增架构解耦进度总览，按 review-clean、已集成、进行中和待实现区分 strict policy、Backend、Project–Graph、Execution、Presentation 与 Frontend Application 工作。
- [ ] 记录 `architecture-final`、各隔离 worktree、最新验证证据、当前 Backend Task 5b 阻塞及剩余跨计划依赖顺序，明确不把未提交草稿或兼容路径纳入集成。
- [ ] 按内容分层提交本次 Assistant、执行输出、编辑器交互、架构文档与 TODO 改动，并推送到 `origin/shadcn`。
- [ ] 为运行时输出补充结构化 `sourcePort`，同步 Rust/TypeScript execution wire、投影与契约测试。
- [ ] 统一 Output、Diagnostics、Pin result search 与 Node detail 的节点/Pin 语义化显示，并新增画布节点选择器交互。
- [ ] 接入禁用发送的 Assistant UI Workbench Shell，注册 root Dockview、布局持久化、View 菜单及中英文文案。
