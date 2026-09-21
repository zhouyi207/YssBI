# JSON Driver：语义组件驱动页面计划

> Status: Planned
> Scope: 语义组件目录、JSON 页面描述、数据绑定、受控动作、AI 生成和增量更新
> Canonical owners: 本文维护未完成目标；当前报告能力由 Graph 与 Execution 及 Results 源码维护
> Update when: 页面范围、组件契约、AI 接入、增量协议或验收状态改变时

原始方案见 Git 提交 `8fa05bf4` 中的 `docs/architecture/jsonDriver.md`。它提出的是 **JSON 页面描述 → 组件目录 → React 渲染**，包含 AI 生成与局部更新。后续线性回归报告的章节配置只实现了其中很窄的展示场景，不能代表整份方案完成；本计划继续保留。

## 原始目标

先定义可以使用的语义组件模板、props、events/actions 和 children，再由 JSON 组合页面。AI 只能生成目录允许的结构，不能生成任意 HTML、JavaScript 或未经登记的操作。

候选组件包括结果表、统计摘要、图表、表单、Markdown 和布局容器。组件目录同时提供 React 实现、输入校验、动作约定及供 AI 使用的描述。数据绑定引用已有结果或资源；统计值由 Rust 与 Results 提供，不能把模型生成的数值当作计算事实。

页面需要局部变化时，只更新受影响的组件和绑定。流式生成、动作响应和模板复用需要自己的身份、安装与恢复语义，不复用 Graph 编辑补丁作为 UI 协议。

## 当前实现与缺口

| 能力                                       | 当前状态与依据                                                                                                                                         |
| ------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 线性回归报告章节配置                       | 已有局部实现：[LinearRegressionReportSpec](../../src/shared/types/domain/linearRegressionReportSpec.ts) 只允许固定报告类型、当前结果引用和平面章节列表 |
| 报告 JSON 导入、导出与校验                 | 已接入[布局控件](../../src/modules/results/internal/ui/info/LinearRegressionReportLayoutControls.tsx)，可调整顺序和显示状态                            |
| 报告呈现                                   | [报告组件](../../src/modules/results/internal/ui/info/LinearRegressionComponent.tsx) 按固定章节类型渲染；配置保存在当前挂载视图的 React state 中       |
| 通用组件目录和页面 Renderer                | 尚未按本方案接入。报告中的固定章节分支不等于可供不同页面与 AI 共用的组件目录                                                                           |
| 通用 props / children / 数据绑定 / actions | 当前报告 Spec 不支持这些页面契约；已有节点配置表单也不能作为通用页面能力的完成证据                                                                     |
| AI 生成、流式安装和局部修改页面            | 尚未接入生产能力；当前 [Harness capability](../../src-tauri/crates/yss-harness-contract/src/lib.rs) 没有生成或修改 UI Spec 的入口                      |
| 跨结果模板与持久化                         | 当前报告布局不能据此跨结果、跨执行会话或重新打开后恢复；尚需定义需求和资源归属                                                                         |

当前已实现的报告契约见 [Graph 与 Execution](../architecture/GRAPH_AND_EXECUTION.md#线性回归语义报告布局)。本次核对只确认源码接入范围，没有执行桌面验收。

## 剩余工作

- [ ] 明确首个 JSON 页面场景和必需组件，区分页面布局、组件数据与工作台布局；用该场景验证完整链路，不能把现有章节排序作为整份计划的验收。
- [ ] 定义封闭的 UI Spec 与组件目录，包括稳定元素 ID、允许的 props/children、数据引用、动作及校验错误。复用现有组件，不默认重写全部页面。
- [ ] 实现目录到 React 的呈现入口，让前端和 AI 使用同一份组件能力描述；未知组件、非法字段和未授权动作必须被拒绝。
- [ ] 接入已有数据查询和 Application 操作，保留项目、资源、结果会话和租约校验；UI Spec 不持有新的业务事实源。
- [ ] 定义增量更新与流式安装协议，明确基线身份、顺序、大小和深度限制、失败回退及重连恢复，保留未变化组件的交互状态。
- [ ] 在 Harness 的受控能力边界内接入 UI 生成与修改，区分展示变更和业务写入；原始 JSON 文本不能直接调用任意 IPC 或执行脚本。
- [ ] 明确是否支持模板复用、保存及重新打开，定义归属和引用重绑定；按当前契约实施，不添加旧格式迁移逻辑。
- [ ] 人工验收组件组合、受限动作、流式/局部更新、错误配置保留上一有效页面，以及项目或结果切换后的迟到更新隔离；测量代表性页面的更新成本。

## 技术选择与边界

原讨论涉及 json-render、Zod、Zustand + Immer、JSON Patch / JsonDiffPatch，以及表单和图表库。这些是候选方案，没有因为早期报告试点而完成选型，也不意味着必须增加这些依赖。实现时先核对现有校验、组件与状态更新能力，再验证确实需要的增量。

GraphDocumentPatch 是业务编辑契约，Graph projection delta 是只读同步契约，UI Spec 及其增量是展示契约；三者即使都可表示为 JSON，也不能直接混用。Workbench 拓扑继续由 FlexLayout 拥有，结果数值继续由 Results 拥有。

[返回专项计划](README.md) · [React / Harness 共用入口计划](motion.md)
