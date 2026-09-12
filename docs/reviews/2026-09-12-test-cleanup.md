# 2026-09-12 测试清理审计

> Status: Historical
> Scope: 按 docs/clear/clear_test.md 扫描测试、评估候选并清理低价值断言与辅助代码
> Canonical owners: 对应源码和保留测试拥有当前行为；本文记录本次取舍
> Update when: 本次清理范围或验证结果补全时

## 范围与判断方法

扫描仓库非忽略的 Rust、TypeScript/TSX、JavaScript/MJS、Python 和 Julia 源文件，共 1,925 个；发现 517 个包含测试入口的文件。入口扫描包含 Rust 内联测试、独立测试文件、插件测试和 Julia testset。参数化声明按入口计，不将源码入口数等同于实际运行用例数。

先扫描全部入口及缺失字段、旧字段、序列化、默认值、导出清单、源码扫描等候选信号，再结合被测实现和已有覆盖审读候选；不按关键词自动删除。数值、Graph 核心语义、持久化、IPC、插件协议、安全、异步身份、撤销与重做测试默认保留，没有证据证明低价值的测试不删除。

[完整扫描清单](2026-09-12-test-inventory.csv)列出文件、测试入口、扫描时行号及最终分类，另列原生插件测试启动器。Rust 入口复用仓库 `source_test_detection.py` 排除注释与字符串中的伪测试声明。清单中的 KEEP 表示没有充分删除依据；它不表示对每个保留测试的全部断言进行了逐行重审。

仓库已有其他未提交改动。本次只修改下表中的测试及对应审计文档，不改 Rust 生产代码或进行兼容逻辑迁移。删除文件已在系统临时目录备份，包括清理前未跟踪的 agGridTheme.test.ts。

## DELETE / MERGE 决策

表中每项均核对外部行为、替代覆盖、是否在重复库行为、实现耦合、复发严重程度和维护成本。没有同等替代测试时明确写出，不以类型检查冒充行为覆盖。

| 分类   | 文件与测试名称                                                                                                                                                                               | 当前保护内容                                 | 删除或合并理由、风险与维护成本                                                                                                                    | 替代覆盖                                                                                                                        |
| ------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------- |
| DELETE | `src/components/data-grid/agGridTheme.test.ts` / `maps the semantic runtime tokens to all grid surfaces and text roles`                                                                      | 内部主题参数逐字段映射                       | 不渲染表格，只重复映射表；内部 token 重组即失败，复发风险为低风险样式偏差                                                                         | 无同等表格视觉测试；保留 `themeTokens.test.ts` 的可读性、主题解析和无效颜色回退                                                 |
| DELETE | `src/shared/theme/chartTheme.test.ts` / `derives chart surfaces and typography from resolved semantic tokens`；`uses the resolved accent for the primary series and fixed semantic statuses` | 私有配色对象结构与字段别名                   | 固定复制实现映射，不验证图表可读性或结果正确性；调整内部色彩角色即需改测试                                                                        | 无同等视觉覆盖；保留主题可读性、图表 renderer 与图表数值域测试                                                                  |
| DELETE | `src/shared/theme/themeTokens.test.ts` / `provides stable semantic pin categories without per-pin settings`                                                                                  | 固定 palette 键名和一个十六进制颜色          | 已移除配置模型的历史清单，无稳定对外 wire；主题调整不应触发此断言                                                                                 | 其余主题解析、无效输入与对比色测试保留                                                                                          |
| DELETE | `src/app/windows/workbench/integrations/workbenchCommandCoordinator.test.tsx` / `exposes only the caller-shaped Workbench command surface`                                                   | 内部组合对象键名和回调对象身份               | 大量 mock/React harness 只重复 TypeScript capability 形状，没有执行用户命令；属于内部编排表示，非外部 API 协议                                    | 保留菜单命令行为和各 Application 用例测试；它们不等同于完整 coordinator 端到端覆盖                                              |
| MERGE  | `src/features/domain/editorProjection/interactionPinData.test.ts` / `strips component capabilities before Pin data enters interaction state`                                                 | Pin 数据无回调且可 structuredClone           | 同一风险已有实际 Pin 组件事件路径覆盖，保留更接近使用方的测试；避免重复 fixture                                                                   | `Pin.preview.test.tsx` / `renders the canvas handle slot without swallowing its pointer event` 已检查回调剥离与 structuredClone |
| MERGE  | `src/utils/appLogger.test.ts` / `does not expose a user-notification logging channel`                                                                                                        | logger 对象没有 notify 属性                  | 单个历史属性黑名单弱于对生产调用的 AST 审计；不删除日志与用户反馈分离契约                                                                         | `userFeedbackArchitectureContract.test.ts` 检查实际生产反馈调用；本文件日志投递行为保留                                         |
| MERGE  | `src/modules/workbench/internal/state/workbenchUiStore.test.ts` / `keeps only non-placement workbench UI state`                                                                              | 默认值和六个历史 Store 字段不存在            | 默认值已由重置测试验证；旧字段拼写不等于状态 authority，不继续维护历史字段名单                                                                    | `updates modal state, then resets it`；前端 state authority 门禁与 Dockview 布局行为测试保留                                    |
| DELETE | `src/shared/types/report/report.test.ts` / `keeps tooltip HTML builders out of the report type boundary`                                                                                     | 已删除 tooltip helper 不再导出               | 只防止历史名字返回，不保护统计报告 wire 或数值                                                                                                    | 保留同文件全部报告解析、Rust fixture 和 malformed input 契约测试                                                                |
| DELETE | `src/tests/architecture/moduleDependencyAudit.test.ts` / `shares the path inventory across sources and avoids resolving the same dependencies twice`                                         | checker 调用次数、缓存命中和内部路径扫描次数 | 绑定私有优化策略；相同审计结论下改变缓存实现也失败。复发风险为审计性能下降，不涉及生产正确性                                                      | 保留下一测试的跨快照移动/删除失效检查，以及 frontendArchitecture 中符号与依赖解析测试                                           |
| DELETE | `src/modules/settings/internal/ui/SettingsView.test.tsx` / `does not offer the removed panel-position appearance setting`                                                                    | 两个旧翻译 key 不出现在页面                  | 历史已删除设置项清单；不验证当前用户操作                                                                                                          | 保留主题选择、平滑滚动、搜索、重置和错误脱敏交互测试                                                                            |
| DELETE | `src/modules/workbench/internal/ui/sidebar/SidebarEmptyState.test.tsx` / `renders a wrapping tab-level state without a scrollbar viewport`                                                   | 静态文案及特定 ScrollArea DOM 结构不存在     | happy-dom 不验证实际换行或滚动，主要重复 React 静态渲染与组件组织；低风险外观调整                                                                 | 无同等 tab 空态视觉测试；保留 section 空态可访问标签和键盘聚焦测试                                                              |
| DELETE | `src/shared/charts/chartArchitecture.test.ts` / `keeps legacy plot compatibility paths removed`                                                                                              | 两个旧图表目录必须永远不存在                 | 只证明历史迁移完成，不保护真实依赖方向；未来目录改名也会触发                                                                                      | 保留真实图表包依赖边界和全局前端架构门禁                                                                                        |
| MERGE  | `src/shared/charts/chartArchitecture.test.ts` / `keeps dependencies inside the approved package boundaries`；`keeps SVG lifecycle and resize observation in chart core`                      | 图表依赖边界与 SVG/ResizeObserver 生命周期   | 保留两项契约，改用现有 TypeScript AST/依赖解析工具；删除手写 lexer、重复源文件枚举和字符串/JSX/泛型解析 fixture，不再维护另一套 TypeScript 解析器 | `typescriptAudit`、`moduleDependencyAudit`、`productionSourceAudit`；图表独有的边界与生命周期断言继续运行                       |

## KEEP 与 REVIEW 结论

以下高疑似候选经复核保留；REVIEW 的处理结果为 KEEP，不以不确定性为由删除。

| 分类          | 范围或代表测试                                                                                          | 保留依据                                                                                                             |
| ------------- | ------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------- |
| KEEP          | Rust 数值、DataFusion / Arrow / Parquet、数据库和 Julia/Bayes 测试                                      | 保护计算结果、数据边界或统计含义；getter/序列化关键词不构成删除理由                                                  |
| KEEP          | `yss-api` 错误 wire、DTO、diagnostics，以及 TS 各 wire parser / Rust golden fixtures                    | 字段 presence/absence 是维护中的严格协议和脱敏契约                                                                   |
| KEEP          | Project、Graph document、clipboard、插件 protocol/SDK 和包签名测试                                      | 文件格式、数据完整性、协议或权限保护                                                                                 |
| KEEP          | Graph Draft、Resolver、Execution、异步请求身份、项目切换与结果 lease 测试                               | 复发可造成错误结果、数据丢失或严重状态不一致                                                                         |
| KEEP          | `databaseRecords.test.ts` / `normalizes machine load state without retaining raw load errors`           | 缺失字段断言用于阻止敏感错误内容泄漏，属于安全边界                                                                   |
| KEEP          | `editorPaneStateStore.test.ts` / `keeps the empty selection snapshot stable for an uninitialized panel` | 引用稳定性虽小，破坏外部 Store snapshot 可导致 React 持续重渲染或崩溃                                                |
| KEEP          | `workbenchRuntime.test.ts` 的 public capability 边界、事务回滚、关闭与 hydration 测试                   | 限制绕过保存流程的布局操作，保护工作台状态与文档安全                                                                 |
| REVIEW → KEEP | `legacy_execution_runtime_and_project_store_mirrors_are_absent`                                         | 除旧路径外，还阻止 ProjectStore 重建 Execution 状态镜像；与仓库唯一 authority 契约相关，不能整项作为历史 trivia 删除 |
| REVIEW → KEEP | `settingsStore.test.ts` 的旧存储 key 清理与已知 appearance 字段投影                                     | 仍有执行中的持久化清理路径；未证明过时且可安全删除，不顺带修改生产兼容逻辑                                           |
| REVIEW → KEEP | 菜单权限、原生窗口装饰、只读数据标签、快捷键抑制、空态可访问性等小测试                                  | 用户可观察行为或重要边界，小并不意味着低价值                                                                         |

## 验证

本次移除 13 个测试声明（含合并到已有覆盖的重复项），删除 4 个完整测试文件。图表架构的两项有效检查保留；手写 lexer、源码枚举、token 类型和重复语法 fixture 随替换移除。删除项独占的 mock、React harness、spy 清理和 import 同步删除；共享 fixture 和生产 helper 仍有引用，继续保留。

对照清理前备份，本次涉及的测试文件从 2,082 行降为 981 行，净减少 1,101 行；未新增测试声明。

- `pnpm check:ts`：通过。
- `pnpm test:ts src/shared/charts/chartArchitecture.test.ts src/tests/architecture/moduleDependencyAudit.test.ts`：2 个文件、3 项测试通过。
- `pnpm test:ts src/utils/appLogger.test.ts src/modules/workbench/internal/state/workbenchUiStore.test.ts src/shared/types/report/report.test.ts src/shared/theme/themeTokens.test.ts src/modules/settings/internal/ui/SettingsView.test.tsx src/modules/workbench/internal/ui/sidebar/SidebarEmptyState.test.tsx src/modules/graph-editor/internal/ui/Pins/Pin.preview.test.tsx src/services/userFeedbackArchitectureContract.test.ts src/app/windows/workbench/menuContributionRegistry.test.tsx`：9 个文件、46 项测试通过。
- 删除文件名在 `src/`、`plugins/`、package/Vite/TypeScript 配置中没有剩余引用。

- `pnpm lint:ts`：通过，仅有 11 条清理前已存在的警告，本次改动无新增警告。
- `pnpm test:ts src/tests/architecture/documentationContract.test.ts -t 'keeps maintained relative Markdown links resolvable'`：文档链接检查 1 项通过，其余 5 项按筛选跳过。合计 50 项聚焦测试通过。
- 对本次修改的测试与文档运行局部 `pnpm format:check:ts`：11 个文件通过；`git diff --check` 通过。

未改 Rust 或插件测试，未运行整个 Rust workspace 或全部前端测试；本次验证是聚焦验证。

## 第二轮：提高保留标准

在首轮清单基础上，继续删除只验证赋值、内部表示、默认示例或低风险标题布局的测试；无分支转发函数不再按每种返回状态重复枚举。关键命令路由、输入校验、数据正确性、异步生命周期和可访问性仍然保留。以上首轮统计与验证记录保持历史含义，下表为本轮增量。

本轮删除 8 个测试声明，包含一个展开为 6 个用例的参数化声明，即减少 13 个运行用例；删除 2 个完整测试文件，净减少 127 行测试代码。两轮累计移除 21 个测试声明、6 个完整测试文件。没有修改生产逻辑。

| 决策   | 文件 / 测试                                                                                                                        | 原保护内容与删除依据                                      | 替代覆盖或明确接受的缺口                                                                                                       |
| ------ | ---------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------ |
| DELETE | `src/features/application/settings/appearanceRuntime.test.ts` / `applySmoothScrollSetting toggles html data attribute`             | 重复一行 dataset 赋值；没有测试实际滚动效果。整个文件删除 | 不再单独断言属性赋值；不声称已有实际滚动效果的替代测试                                                                         |
| MERGE  | `src/features/core/dataStore/pinLinks.test.ts` / `derives connected state from pinConnections ids`                                 | 内部数组长度和对象包装，独立文件维护收益低。整个文件删除  | `nodeView.test.ts` 已在节点投影消费路径检查 connected 和 connectionIds；不再单独枚举 undefined 包装与 linkCount                |
| DELETE | `src/shared/theme/themeTokens.test.ts` / `derives surfaces and interaction tokens from a theme palette`                            | 固定颜色字段拷贝和 color-mix 字符串形式，重构耦合高       | 同文件保留前景色可读性及非法颜色回退；不再冻结内部 token 表示                                                                  |
| DELETE | `plugins/julia/web/src/modules/bayes/internal/ui/components/BayesPanels.test.ts` / `provides one canonical LaTeX default formula`  | 冻结默认示例的符号名单，并未验证公式计算正确性            | 同文件保留已有表达式编辑、完整公式左侧、统计阈值与图表投影；默认符号名单不另设替代断言                                         |
| DELETE | `src/modules/graph-editor/internal/ui/Pins/Pin.preview.test.tsx` / `maps structured feedback to safe metadata`                     | 仅检查内部 data 属性映射，未执行连接验证或安全检查        | 同文件保留真实 Pin 指针事件和预览生命周期；不再冻结 CSS 元数据形式                                                             |
| DELETE | `src/modules/output/internal/ui/RunOutputPanel.test.tsx` / `renders a shared header without exposing the focused graph path`       | 标题文字及路径摆放属于低风险布局约束                      | 同文件保留输出顺序、清理和运行失败定位；标题布局不另设替代断言                                                                 |
| DELETE | `src/modules/problems/internal/ui/GraphProblemsPanel.test.tsx` / `renders a shared header without exposing the focused graph path` | 仅冻结标题及计数字样和路径摆放，不是保密边界              | 同文件保留权威诊断投影、空态和语义位置展示；标题布局不另设替代断言                                                             |
| MERGE  | `src/features/application/editor/edgeOperations.test.ts` / `propagates the typed reroute outcome unchanged: %j`                    | 6 种状态经过同一条无分支 return 路径，重复 mock 转发      | 已有 `copies the position and sends one typed InsertReroute intent` 改为检查返回对象身份；命令路由、非法参数和坐标拷贝测试保留 |

本轮所有候选均对照实现与调用者复核：没有把通用库行为当作业务契约，也没有以测试体积小作为唯一删除理由。Edge 命中路径测试仍保护几何一致性；连线命令测试仍保护不同意图的路由；Bayes 符号转换与统计结果测试仍有独立行为意义，继续保留。

删除项独占的 import、空 describe、属性清理 hook 随用例一起移除；生产 helper 仍有调用，不删除。修改前备份保存在系统临时目录 `yssbi-test-cleanup-round2-qso7e3rp`。

### 清单修正

CSV 仍保留首次扫描的 2,092 行记录（包含原生测试启动器），`scan_line` 是首次扫描位置，不是当前行号。现有决策合计 KEEP 2,069、DELETE 16、MERGE 7；MERGE 包含首轮保留并重写的两个架构测试，因此不能直接用 DELETE + MERGE 作为移除数量。本轮参数化删除对应 `edgeOperations.test.ts` 中扫描位置最大的参数化声明。

发现原 CSV 的 `basis` 字段已全部变为问号，原文无法从文件恢复。未重新审查的行保留原决策，并明确注明编码损坏、指向本报告首轮依据；本轮 8 项写入具体可读理由。CSV 使用 UTF-8 BOM 保存，未伪造损坏字段的原始内容。

### 本轮验证

- `pnpm test:ts src/shared/theme/themeTokens.test.ts src/features/core/dataStore/nodeView.test.ts src/features/application/editor/edgeOperations.test.ts src/modules/graph-editor/internal/ui/Pins/Pin.preview.test.tsx src/modules/output/internal/ui/RunOutputPanel.test.tsx src/modules/problems/internal/ui/GraphProblemsPanel.test.tsx`：6 个文件、27 项测试通过。
- `pnpm test:plugin:julia plugins/julia/web/src/modules/bayes/internal/ui/components/BayesPanels.test.ts`：1 个文件、8 项测试通过。
- `pnpm check:ts`：通过。

- `pnpm check:plugin:julia`：通过。
- 对本轮 6 个修改后的测试文件及本报告运行局部 `pnpm format:check:ts`；对本轮文件运行 `git diff --check`。
- CSV 核对：历史记录 2,092 行、本轮具体决策 8 行、无纯问号 basis；删除文件名在源码和构建配置中无剩余引用。

未运行 Rust workspace 或全部前端测试；本轮仅修改宿主与插件前端测试及清理记录。
