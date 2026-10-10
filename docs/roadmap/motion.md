# motion：GPUI / Harness 共用入口与投影交付

> Status: Planned
> Scope: 各领域共用 Application、类型化投影、界面意图与剩余验收
> Canonical owners: 本文维护覆盖与进度；当前契约由 Application、GPUI、Graph、Harness、Project 与 UI Contract 维护
> Update when: 操作覆盖、交付方式、界面意图或验收状态改变时

GPUI 和 Harness 都消费已有 Application 能力。业务状态留在原 Rust 领域，宿主把阻塞用例交给 worker，
接纳类型化回执与事件。根 DockArea 唯一拥有工作台拓扑，界面意图只请求动作，不另存布局。
当前契约见 [Application](../../crates/yss-application/README.md)、[UI Contract](../../crates/yss-ui-contract/README.md)
和 [GPUI host](../../crates/yss-desktop-gpui/README.md)。

## 当前覆盖

| 领域 | 原生 GUI | Harness | Owner / 交付 |
| --- | --- | --- | --- |
| Graph | 编辑、历史、显式保存、运行与结果 | 定向查询、节点/连接/常量工具、校验、执行与保存 | Project 当前文档/历史；Graph 语义；Execution 运行/结果；共用 Application |
| Project 资源 | 项目生命周期、资源菜单与目录 | 列表/检查、创建、重命名、复制、删除和保存等领域工具 | Project 资源权威；项目生命周期权限不由资源工具自动扩大 |
| Data | 导入、分页、只读表格、类型/语义、checkpoint、导出 | 概览/schema/profile/分页、获准行列批量编辑、导入导出和历史 | Database / Project；GUI 不因模型写能力存在就具备行编辑入口 |
| Chart | 配置草稿、版本化预览与显式保存 | inspect_chart / update_chart | Project 文件；Application 用例；模型更新遵循原立即持久化约定 |
| Doc / Mind | 未提交输入、应用/保存、资源导航 | 定向查询、文本/主题操作和显式保存 | Project 与模型校验；原生表单不拥有持久正文 |
| Results | 保留、分页、报告/分析/图形及独立窗口 | 列表、概览、表格读取和打开意图 | Execution ResultStore；完整结果身份与租约 |
| Plugin | 安装、启停、原生视图与任务管理 | 不由已有资源工具推断插件管理写权限 | Plugin Manager / Runtime |
| 界面意图 | 认领、串行执行、完成确认 | request_ui_intent / inspect_ui_intent | Application 请求与回执；宿主执行原打开/定位/面板入口 |

工具精确目录由 [Harness Core](../../crates/yss-harness-core/README.md)拥有，阶段及界面验收见
[领域工具计划](HARNESS_TOOL_ARCHITECTURE.md)。Data/Chart 写能力已有接入，不再列为从零新增。

Graph 的 GUI 显式 Save 与 Harness 批次原子保存保持差异。图通知后重读、有界运行事件、按需结果查询、
项目发布与界面意图是不同数据流，不合并成可任意修改的全局 JSON Store。

## 已接入

- [x] 既有 GUI/Harness 用例与原领域 owner 共用；原生宿主直接消费类型化入口。
- [x] 受控打开资源、定位图节点、打开保留结果与显示面板复用现有实体和租约。
- [x] 意图按调用者/clientKey 去重，面向 main 工作台，回执区分 pending/claimed/applied/failed/expired。
- [x] Application 校验项目/会话/结果；宿主认领后串行执行，旧 binding 不得结算新队列。

这些实现记录不替代完整人工验收；图首次读取、通知缺口与重连的具体局部证据见组件审查。

## 剩余工作

- [ ] 人工验收重复意图、打开/定位、面板关闭重开、会话替换、迟到投影与恢复；同一目标不同生命周期不能串用回执。
- [ ] 对 Graph 通知后刷新和直接增量交付做同场景端到端测量，再决定是否调整协议；处理、布局、绘制分别计量。
- [ ] 已有 Project 资源、Data、Chart 写工具完成原生展示与失败/恢复验收；项目生命周期、Plugin 等额外写能力仅在明确产品需求与授权后扩展，沿用幂等和提交判据。
- [ ] 按需评估意图取消、更多资源方式及多工作台目标；超时只表示没有完成证据，不代表正在执行的界面动作已取消。
- [ ] CLI / MCP 及后台客户端遵循 [Harness 路线图](STATISTICAL_HARNESS.md)的准入，不因内部工具存在就标记生产接入完成。

报告与结果验收见 [GPUI 组件审查](GPUI_COMPONENT_AUDIT.md)和[组件重构验收](COMPONENT_REFACTOR.md#102-界面人工验收)。
从仓库根 `cargo run`；本次文档整理未运行检查或交互验收。

[返回专项计划](README.md)
