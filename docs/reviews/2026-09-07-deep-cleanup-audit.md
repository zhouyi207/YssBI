# YssBI 深度清理审计 · 2026-09-07

> Status: Historical
> Scope: 基于 ccd31697 的架构、命名、无效逻辑与历史遗留审计
> Canonical owners: 源码和测试拥有实现事实；本文保存本次检查证据，不替代当前架构文档
> Update when: 本次交付补充验证结果；后续实现变化另行记录

项目需要清理。主要问题是迁移后没有完整退出的旧契约、没有行为消费者的设置、仍被调用的空实现，以及模块之间的循环依赖。Rust authority、Graph Draft 与 committed state 分离、独立的 Results/Problems/Logs 边界已有明确实现，清理应保留这些边界。

**检查范围与证据口径**

- 基线工作区干净，HEAD 为 `ccd31697`。读取了 `.rules`、当前架构、Graph/Execution、Workbench、IPC、开发和验证规范。
- Cargo metadata 检查覆盖 73 个 workspace package。除桌面根包外，各包都有非 dev 的 workspace 依赖方；没有据此发现整包孤立的废弃 crate。
- 前端导入清查覆盖 999 个 TS/TSX/JSON 候选生产模块。从 `src/app/main.tsx` 跟踪静态导入、字面量动态导入、类型导入和再导出，再用已安装 TypeScript 7 的 AST 排除显式 type-only 边，复核主要循环链。
- 导出符号词法扫描产生 61 个“生产源码只出现一次”的候选。这不是可直接删除的数量：其中包含测试支持函数、公开能力对象和需要进一步确认的接口。
- 人工深入检查项目加载/发布、Graph draft/history、Workbench、Results/report、设置、数据库 I/O、Rust 身份与资源命名等热点。没有逐行审查全部源码，也没有执行所有科学算法、Julia 或桌面交互测试。
- P1 表示应优先处理的用户行为风险；P2 表示明确的行为、契约、资源占用或结构问题；P3 表示维护性清理。另列一个已在接口级复现、尚未确认常规 UI 触发路径的风险。

**问题清单**

| 编号 | 优先级 | 发现                                                      | 证据等级                   |
| ---- | ------ | --------------------------------------------------------- | -------------------------- |
| F01  | P1     | 自动保存等设置只有存取和控件，没有对应行为                | 全生产源码引用与调用链核对 |
| F02  | P2     | 撤销历史无容量限制，并保存不会被恢复流程读取的旧投影      | 生产读写链核对             |
| F03  | P2     | 4 组非 type-only 导入环，最大涉及 25 个文件               | AST 与具体导入链核对       |
| F04  | P2     | 项目加载沿用“完整 ProjectData”旧返回类型，graphs 固定为空 | 类型、构造与消费者核对     |
| F05  | P2     | 拖拽预览保留空实现；引脚等待器失去等待方                  | 生产调用与写入入口核对     |
| F06  | P2     | 报告解析器放过损坏的嵌套数据，并存在两套校验入口          | 最小复现及源码核对         |
| F07  | P2     | DataSeries 结果只显示默认第一页，没有翻页入口             | renderer 与分页 hook 核对  |
| F08  | P2     | opaque 资源路径仍由前端解析类别和判断有效性               | 前后端契约对照             |
| F09  | P3     | 少数大模块同时承担多项可分离职责                          | 按职责复核，未仅凭行数判定 |
| F10  | P3     | Windows 原子文件替换实现复制了三份                        | 三处实现逐段对照           |
| F11  | P3     | 无消费者依赖、导出和旧常量继续保留                        | 引用核对；候选分级         |
| F12  | P3     | 界面和文档中仍有过时实现描述                              | 前后端及文档门禁对照       |

**F01：设置页提供了实际不生效的功能。**

[SettingsView.tsx](../../src/modules/settings/internal/ui/SettingsView.tsx#L273) 的编辑器区域展示九个设置：`showGrid`、`autoSave`、`snapToGrid`、`fontSize`、`openSideBySideDirection`、`splitOnDragAndDrop`、`alwaysShowEditorActions`、`closeEmptyGroups`、`splitSizing`。它们在生产源码中的使用停留在类型、默认值、设置存取与设置页，没有控制 Canvas、保存或 Dockview 行为的消费者。`projectName`、`exportPath` 也只在客户端设置中存取。

尤其是 [settings.ts](../../src/shared/config-default/settings.ts#L48) 将 `autoSave` 默认设为 `true`，而 [saveGraphDraft.ts](../../src/features/application/graphDraft/saveGraphDraft.ts#L16) 及其调用链没有读取这个开关。用户看到自动保存已开启，却仍需显式保存；这比单纯的命名问题更应优先处理。没有据此声称已经发生用户数据丢失。

建议逐项决定删除未实现的入口或接入实际 owner。自动保存涉及草稿与文件提交语义，应作为明确行为变更处理。已经有行为消费者的外观设置，如 `smoothScroll`、语言和主题，不在这个删除清单内。

**F02：历史栈保存了无用的解析投影，而且没有边界。**

[graphDraftStore.ts](../../src/features/core/graphDraft/graphDraftStore.ts#L94) 的 `cloneVersion` 同时深拷贝 document 和 projection；第 183 行每次变换直接追加历史版本，没有条数或字节预算。保存、重新安装或清空会释放历史，但持续未保存的编辑会一直增加保留量。

[historyCoordinator.ts](../../src/features/application/graphDraft/historyCoordinator.ts#L39) 恢复时只读取历史 `version.document`，调用 Rust Resolve 获取新投影。Store 的 undo/redo 又用该新投影覆盖历史版本中的 projection（第 304、327 行）。因此，历史中的旧 projection 在当前生产恢复路径里没有用途。

建议先把历史条目缩为恢复必需的文档意图，再定义容量或内存预算。现状保留量随编辑次数和文档/投影大小增长；本次未做大图内存压测，不给出未经测量的 MB 数值。`MAX_HISTORY = 50` 虽然仍在 `src/shared/config-default/ui.ts`，却没有消费者，不能当作现有上限。

**F03：目录分层通过门禁，但模块依赖仍成环。**

排除显式 type-only 边后得到四组：Viewport 4 个文件、Graph draft/projection 3 个文件、项目/发布/结果 25 个文件、统计 actions/hooks 3 个文件。以下均有真实值导入，不能仅用 TypeScript 类型互相引用解释。

```text
projectIOStore
  → projectPublicationCoordinator
  → resourceMutationResult
  → results/index
  → results/runtime
  → projectIOStore

graphProjectionLifecycle → resolveGraphDraft → graphDraftCoordinator
                        ← currentProjectionLocale 的反向导入 ──────┘

statsActions → useHypothesisTestBlock → statsActions
```

主环可从 [resourceMutationResult.ts](../../src/features/application/editorMutation/resourceMutationResult.ts#L1)、[results/index.ts](../../src/features/application/results/index.ts#L45) 和 [runtime.ts](../../src/features/application/results/runtime.ts#L10) 复核：发布用例为失效结果导入聚合入口，该入口又导出渲染组件；结果运行态反向读取整个 ProjectIOStore。较小的图解析环，仅取 locale 就引入了完整生命周期模块。

当前结构增加初始化顺序、测试替身、HMR 和职责定位的复杂度；本次没有证明这些环已导致启动异常。建议先让内部调用直接依赖具体 owner，再把项目身份读取与 ProjectIO 用例分离，把 locale/viewport normalization 等纯函数移到已有的较低层 owner。不要通过增加另一个总聚合入口解决。

**F04：类型与名称仍描述已经退出的“完整前端项目模型”。**

[project.ts](../../src/shared/types/domain/project.ts#L24) 声明 `ProjectData`“包含项目的所有内容”，含 `graphs: Record<string, Graph>`。但 [authoritativeProjectLoadPlan.ts](../../src/features/application/project/authoritativeProjectLoadPlan.ts#L204) 始终构造 `graphs: {}`，再 `as ProjectData`。真正的资源索引、草稿和图投影已发布到各自 owner。

[useProjectPicker.ts](../../src/features/application/project/useProjectPicker.ts#L381)、[useProjectOperations.ts](../../src/features/application/editor/useProjectOperations.ts#L216) 等消费者只判断加载结果是否存在，部分消费者直接忽略返回值。这使人误以为加载函数返回可供业务读取的完整项目快照，实际只是旧返回结构继续存活。

建议根据消费者需求改成明确的加载结果/回执，移除虚构的空图集合，然后按引用检查退役 `Graph` 等旧模型。不要先给这些模型统一改名，再继续保留无用结构。

命名还存在可直接改进的例子：[database.rs](../../src-tauri/crates/yss-application/src/database.rs#L94) 的 `ApplicationDatabaseError` 包装 [database/error.rs](../../src-tauri/crates/yss-application/src/database/error.rs#L78) 的 `DatabaseApplicationError`，两者只交换词序却表达不同层次。可按会话/用例错误与数据库操作错误命名。数学中的 `n`、`p`、`beta` 等常规符号不应机械展开。

**F05：已退出的拖拽机制还占着调用链。**

`src/features/application/editor/useEditorDragPreviewMonitor.ts` 的 `clearEditorDragSession`、`useEditorDragPreviewMonitor` 都是空函数，Host 固定返回 null；[editorDragDropActions.ts](../../src/features/application/editor/editorDragDropActions.ts#L109) 仍有七处调用清理函数。[CanvasDropZone.tsx](../../src/modules/graph-editor/internal/ui/Canvas/core/CanvasDropZone.tsx#L1) 的注释还描述已经不存在的预览链。

另一处是 `src/features/core/canvas/pinOffsetWaiter.ts`：唯一会向等待队列添加元素的 `waitForPinOffset` 没有生产调用方，但 [useCanvasViewport.ts](../../src/features/application/editor/useCanvasViewport.ts#L127) 仍在测量后通知等待器。队列在当前生产路径中始终为空。

这两组属于明确的删除候选：一起移除失效入口、导入、通知/清理调用和旧注释，复用现有拖拽/viewport 测试验证实际交互。仅删除函数体或保留兼容空壳会继续掩盖问题。

**F06：报告解析的“已校验类型”不可靠。**

[parseVec.ts](../../src/shared/types/report/parseVec.ts#L30) 只验证 coefficients、equations、beta 等是数组，再直接放入有具体元素类型的返回值；[guards.ts](../../src/shared/types/report/guards.ts#L44) 的 `assignPresentKeys` 还把未逐字段校验的嵌套块写入已声明类型的对象。VAR、Panel、回归诊断等相邻解析器也应按实际字段消费检查，不能只修 VEC 一个字段。

最小复现把其它必需字段填为合法值，仅设置 `coefficients: [null]`：期望返回 null，实际返回完整对象。进入 [VECComponent.tsx](../../src/modules/results/internal/ui/info/VECComponent.tsx#L16) 后，`coefficients.map(x => x.eq_name)` 会访问 null。此处验证的是边界拒绝损坏数据的能力，没有证明当前正常 Rust 计算会产生这个值。

此外，[reportValidation.ts](../../src/shared/types/report/reportValidation.ts#L118) 对 OLS 另写一套字段校验；非 OLS 成功路径会调用 `parseReportPayload` 两次。建议让一个解析器同时产出 typed value 或字段级错误，再供界面使用，避免一套宽松解析加一套局部严格校验。

**F07：DataSeries 分页能力没有接到界面。**

[DataSeriesResultView](../../src/features/application/results/components/renderers/ResultRenderers.tsx#L45) 调用 [usePagedResultRows](../../src/features/application/results/usePagedResultRows.ts#L14)，默认每页 200 项，然后仅渲染 `paging.values`；没有使用该 hook 的翻页动作，也没有传入 toolbar。同文件中的 Sequence renderer 已有完整分页工具栏。

因此，超过 200 项的 DataSeries 在这一 renderer 中只能访问第一页。建议复用现有结果分页工具栏，保留后端分页。这是源码可确定的可达 UI 行为缺口，本次未启动桌面应用手动复现。

**F08：资源身份的 opaque 约定与前端实现冲突。**

[graphResourcePath.ts](../../src/shared/types/domain/graphResourcePath.ts#L40) 从 `events/`、`functions/` 推断领域类型；`isValidGraphResourceTabId` 实际只检查该前缀。[openGraphInEditor.ts](../../src/features/application/editor/openGraphInEditor.ts#L28) 即便已经收到显式 `type`，仍从字符串推断并验证一次。

Rust 的 [GraphResourcePath](../../src-tauri/crates/yss-graph-document/src/resource_path.rs#L41) 则验证目录、扩展名、层级和资源名。这是两种不同强度的“有效路径”判断，且与 `.rules` 和当前架构要求的前端 opaque 身份处理不一致。

建议类别取自 Rust 索引/DTO；前端仅处理展示和不改变身份的传递。`NodeId`、`PinId`、`GraphPath` 等在 [ids.ts](../../src/shared/types/domain/ids.ts) 全部是 string alias，应在高风险边界评估不可互换的类型约束，而不是把它们当成已有编译期隔离。

**F09：模块应按具体职责收口，而非按行数机械切割。**

| 文件                                                                                                       | 当前规模与职责                                                                             | 建议优先检查的边界                                                                |
| ---------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------- |
| [workbenchDockviewInternal.ts](../../src/modules/workbench/internal/dockview/workbenchDockviewInternal.ts) | 2,083 行；元数据转换、shadow transaction、序列化布局、live Dockview 操作、排队和 hydration | 纯元数据/布局转换、临时事务执行与 live runtime；保留 root Dockview 唯一 authority |
| [database.rs](../../src-tauri/crates/yss-application/src/database.rs)                                      | 1,493 行；会话校验、导入导出、SQL/Excel 枚举、物理 ingest、临时文件与 Windows FFI          | 用例顺序和会话校验留在 Application，具体 I/O 交给相应 adapter                     |
| [graph-analysis/lib.rs](../../src-tauri/crates/yss-graph-analysis/src/lib.rs)                              | 2,150 行，最终测试模块之前约 1,498 行；语义事实类型、组装与分析逻辑                        | 在当前 crate 内区分事实类型与解析组装，避免再引入第二份 semantic model            |

行数包含空行/注释；Rust 总行数也包含测试，不作为纯生产逻辑行数。`bayes.rs` 的 1,755 行也有约 611 行最终测试模块，不能全部计为单体业务逻辑。

`yss-application`、`yss-api` 分别直接依赖 48、37 个 workspace 包；这是依赖面观察，不能单独证明层次错误。具有多个调用方的 naming/hash/identity 小 crate 不应仅因体积小而合并。

**F10：平台文件替换有三份重复实现。**

[database.rs](../../src-tauri/crates/yss-application/src/database.rs#L1399)、`window-state/persistence.rs`（历史实现，现已由官方窗口状态插件替换）、[julia-worker/assets.rs](../../plugins/julia/native/crates/yss-julia-worker/src/assets.rs#L159) 都重复编码 Windows 路径、调用 `MoveFileExW`、设置 replace/write-through 标志并读取系统错误。

后续修正 Windows 行为需要维护三处。建议先区分共同的平台 primitive 与各自的临时文件/错误策略，再确定复用位置；不要让 Window/Julia adapter 为复用而依赖 Project 领域，也不要把业务事务抽成万能文件工具。

**F11：可缩减的导出、依赖和旧常量。**

- [package.json](../../package.json#L53) 的 `marked` 未发现 TS、CSS 或构建/脚本生产消费者。实际 Markdown 渲染走 `react-markdown`。删除时同时修改 lockfile 和架构依赖策略；`shadcn`、字体和 `tw-animate-css` 有 CSS 使用，不能随同删除。
- [statsActions.ts](../../src/features/application/stats/statsActions.ts#L66) 的 `useStatsBlock` 只是 `useHypothesisTestBlock` 的无消费者别名，泛化名称掩盖实际功能。
- [canvas.ts](../../src/shared/config-default/canvas.ts) 的 `CANVAS_DEFAULT_SCALE`、`NODE_WIDTH`、`NODE_HEIGHT` 和 `src/shared/config-default/ui.ts` 的三个常量都未发现生产消费者。移除前先确认正确的现有行为 owner，不把旧常量重新接回去制造行为变化。
- `parseDatabaseGridClipboard`、[parseGraphResourceUri](../../src/shared/types/domain/graphResourcePath.ts#L57) 有测试但没有生产消费者。它们属于“尚未接入或已退出”的决策项，不能用测试存在证明功能仍被使用。
- `src/shared/charts/index.ts` 没有入口可达引用，具体图表却被直接导入；它是 barrel 清理候选。[commandExecutor.typecheck.ts](../../src/features/application/graphEditing/commandExecutor.typecheck.ts) 是有意的编译期断言，不应按运行时不可达删除。

61 个单次出现导出仅是候选库存。`ForTests` seam、公开 capability 或动态访问接口需另行确认，本文未把它们计为 61 个死函数。

**F12：过时描述仍在引导维护者。**

[VECComponent.tsx](../../src/modules/results/internal/ui/info/VECComponent.tsx#L275) 的空结果分支提示“VEC 协整估计尚未实现”。但 [vec/estimate.rs](../../src-tauri/crates/yss-sci/src/ts/vec/estimate.rs#L25) 已调用 Johansen stage，且 [node_statistics.rs](../../src-tauri/crates/yss-sci-runtime/src/time_series/models.rs) 已接入 `vec_estimate`。建议删除旧实现状态判断，按真实空数据/失败状态表达；这不等于宣称所有 VEC Graph kernel 已接通。

初始架构检查发现 Module Map 仍列出已删除的 `src/modules/plugins`，导致两项文档测试失败。本次已用 `pnpm docs:module-map` 重新生成，仅删除该过期行。`yss-application` README 把已经存在的 yss-api 适配层写成未来工作，本次同步纠正这句文档。业务代码中的遗留项留在清单中，未批量改写。

**R01：共享结果释放缺少消费者身份，生产触发范围待确认。**

[useResultValue.ts](../../src/features/application/results/useResultValue.ts#L65) 和 [usePagedResultRows.ts](../../src/features/application/results/usePagedResultRows.ts#L79) 在任一消费者卸载时按 resultId 调用 `releasePayload`。[resultQueryCoordinator.ts](../../src/features/application/results/resultQueryCoordinator.ts#L303) 会使该结果的所有 value/page 请求失效，[runtime.ts](../../src/features/application/results/runtime.ts#L65) 会删除共享缓存，未区分仍然挂载的消费者。

最小复现使用真实 hook 和 coordinator，publication 模拟 runtime 相同的删除/通知语义：两个消费者读取同一 scalar 42，卸载 A 后，仍挂载的 B 变成 `empty`。该问题已经在接口级证明。

但 [upsertResult](../../src/modules/workbench/internal/dockview/workbenchDockviewInternal.ts#L1741) 当前按 resultKey 去重，独立窗口也有自己的前端运行环境；本次没有证明常规 UI 操作能同时挂载两个同 resultId 的读取者。因此将其列为需确认的不变量风险，不宣称“关闭任意结果窗口会清空另一个窗口”。若允许共享消费，应引入消费者生命周期/引用计数或局部 payload；若只允许单消费者，应明确并验证该限制。

**清理顺序与验收目标**

1. 先处理会误导或限制用户的行为：F01 自动保存/无效设置、F06 损坏报告边界、F07 DataSeries 分页。每个改动用实际交互或边界回归证明。
2. 删除已证明无行为的残留：F05 空预览与等待器、F11 确认的别名/常量/依赖、F12 旧文案。不要用新增兼容 facade 保留旧调用。
3. 收紧状态与结果：F02 历史文档存储/容量、F04 加载结果类型，再明确 R01 的消费者数量约定。
4. 最后整理依赖与职责：按 F03 的具体循环链逐一解除，调整 F08 身份传递，再处理 F09/F10 的职责分离与平台重复。每次保持一个可回顾的责任边界，避免一次大规模移动目录。

**实际验证与本次改动**

| 检查                                                                                                                                                                                       | 结果与范围                                                                                                    |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------- |
| `pnpm check:ts`                                                                                                                                                                            | 通过；基线前端类型检查                                                                                        |
| `pnpm lint:ts`                                                                                                                                                                             | 退出码 0；14 条警告，其中 5 条位于测试文件，其余为冗余 fallback/spread 等问题                                 |
| `pnpm test:architecture`                                                                                                                                                                   | 基线 56 通过、2 失败，7 个文件；两个失败都来自过期 Module Map，耗时约 132 秒                                  |
| `pnpm test:ts src/features/application/results/resultQueryCoordinator.test.ts src/features/application/results/resultReadErrors.test.tsx src/features/application/results/runtime.test.ts` | 在与临时复现文件同次执行中，三个现有文件的 10 项测试通过                                                      |
| `pnpm test:ts src/shared/types/report/report.test.ts`                                                                                                                                      | 在与临时复现文件同次执行中，现有 18 项测试通过                                                                |
| 临时最小复现                                                                                                                                                                               | 两项期望正确行为的断言均失败：VEC 接受 `[null]`；共享读取者从 42 变为 empty。复现源码附后，临时测试文件已移除 |
| `pnpm test:rs:package -p yss-graph-document -p yss-project-identity -p yss-resource-naming --lib`                                                                                          | 三个 package 的 9 项测试全部通过；覆盖文档/语义哈希、身份/revision、资源命名                                  |
| `pnpm test:ts src/tests/architecture/documentationContract.test.ts`                                                                                                                        | 文档修正后 6 项全部通过；原有两项文档失败已消除                                                               |
| 局部文档格式与 `git diff --check`                                                                                                                                                          | 三份修改文档已局部格式化；`git diff --check` 通过                                                             |

未执行完整 `pnpm run ci`、Rust 根包 production architecture test、Rust 全 workspace 编译/Clippy/测试、Julia 测试或 Tauri 桌面手动验证。通过的聚焦检查不代表这些范围已经验证。架构检查能够保护已有分层规则，但通过它不等于设置存在行为消费者、报告嵌套字段可靠或所有模块无环。

本次保留的是审计报告、文档索引及两处文档修正；没有修改生产 TS/Rust 逻辑，没有提交 Git commit。

**最小复现源码**

将下列内容临时保存为 `src/tests/deepCleanupAudit.repro.test.tsx`，运行 `pnpm test:ts src/tests/deepCleanupAudit.repro.test.tsx`。这是记录缺陷的复现，基线预期为两项断言失败；不应在未修复行为时加入常规通过门禁。

```tsx
// @vitest-environment happy-dom
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { expect, it, vi } from "vitest";
import { parseVecSummaryResultData } from "@/shared/types/report/parseVec";
import { useResultValue } from "@/features/application/results/useResultValue";
import {
  createResultQueryCoordinator,
  type ResultQueryReadCapability,
} from "@/features/application/results/resultQueryCoordinator";
import type { ResultValue } from "@/features/application/results/types";
import type { DeepReadonly } from "@/shared/types/deepReadonly";

vi.mock("@/features/application/results/runtime", () => ({
  resultQueryCoordinator: {},
  resultQueryRead: {},
}));

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

it("rejects a malformed nested VEC coefficient before rendering", () => {
  const raw = {
    title: "VEC Summary",
    var_names: ["y"],
    num_observation: 10,
    rank: 1,
    lags: 1,
    trend_spec: "constant",
    equations: [],
    coefficients: [null],
    beta: [[1]],
    cointegrating_equations: [],
    log_likelihood: 1,
    aic: 1,
    hqic: 1,
    sbic: 1,
    det_sigma_ml: 1,
  };
  expect(parseVecSummaryResultData(raw)).toBeNull();
});

it("keeps a shared result available when one of two consumers unmounts", async () => {
  const values = new Map<string, DeepReadonly<ResultValue | null>>();
  const listeners = new Set<() => void>();
  const notify = () => listeners.forEach((listener) => listener());
  const value: ResultValue = {
    kind: "value",
    value: 42,
  };
  const read: ResultQueryReadCapability = {
    subscribe: (listener) => {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
    getValue: (id) => values.get(id) ?? null,
    getDescriptor: () => null,
    getPage: () => null,
    getPinResult: () => null,
    getFailure: () => null,
  };
  const coordinator = createResultQueryCoordinator({
    readCurrentProjectInstanceId: () => "project-1",
    service: {
      getValue: async () => value,
      getDescriptor: async () => null,
      getPage: async () => null,
      getPinResult: async () => null,
    },
    publication: {
      // Same deletion and notification semantics as results/runtime.ts.
      releasePayload: (id) => {
        values.delete(id);
        notify();
      },
      publishValue: (_project, id, next) => {
        values.set(id, next);
        notify();
      },
      publishDescriptor: () => {},
      publishPage: () => {},
      publishPinResult: () => {},
      publishFailure: () => {},
    },
  });
  const dependencies = { coordinator, read };
  function Consumer({ id }: { id: string }) {
    const state = useResultValue("r1", dependencies);
    return createElement("span", { "data-consumer": id }, String(state.value?.value ?? "empty"));
  }
  const host = document.createElement("div");
  const root = createRoot(host);
  try {
    await act(async () => {
      root.render(
        createElement(
          "div",
          null,
          createElement(Consumer, { key: "a", id: "a" }),
          createElement(Consumer, { key: "b", id: "b" }),
        ),
      );
    });
    expect(host.querySelector('[data-consumer="b"]')?.textContent).toBe("42");
    await act(async () => {
      root.render(createElement("div", null, createElement(Consumer, { key: "b", id: "b" })));
    });
    expect(host.querySelector('[data-consumer="b"]')?.textContent).toBe("42");
  } finally {
    act(() => root.unmount());
  }
});
```
