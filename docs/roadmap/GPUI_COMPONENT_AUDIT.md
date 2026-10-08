# GPUI 组件逐项迁移审查

> Status: Planned
> Scope: 所有 Tauri/React 参考组件的必要性、原生覆盖、架构优化与验收
> Canonical owners: React 源码定义参考行为；GPUI 和业务模块 README 定义当前契约；本表只记录逐项审查与开放工作
> Update when: 每次完成组件审查、迁移、优化或人工验收时

用户目标是逐个检查全部参考组件：必要的能力迁入 GPUI，已迁移部分复核效率与职责边界，每批完成后独立提交。
不以名称相似、存在原生文件、编译通过或完成部分功能代表该组件已完整迁移。

当前清单以 `react/src/**/*.tsx` 的生产文件为入口，共 265 项；85 个测试 TSX 文件作为行为证据，不迁成 UI 单元测试。
每个文件的导出组件、内部子组件及其调用的 hooks/服务均需在实际审查时核对；非 TSX 注册、样式、平台适配也随对应组件检查。
仓库已无 `src-tauri` 宿主源码；平台功能按 Application/GPUI 当前入口及 React 平台调用核实。

审查结论使用：**待查**、**迁移**、**优化**、**复用原生组件**、**无需迁移**。无需迁移必须说明当前产品行为或原生替代依据。
实现与验收分别记录：已有源码不自动算作完整覆盖；需要人工验收的行为保持待验收，只有实际证据才更新为通过。
框架组件优先复用 `gpui-component`，业务用例继续调用现有 Rust owner，根 DockArea 持有唯一工作台拓扑。

验证按受影响模块选择契约测试、原生编译/Clippy 和局部格式检查；UI 使用人工验收，不增加 UI 单元测试。
历史迁移和平台验收见 [GPUI 迁移](GPUI_MIGRATION.md)，当前原生契约见 [GPUI host](../../crates/yss-desktop-gpui/README.md)。

## 已审查批次

### 项目知识库与共用设置结构

- 项目知识库设置已接入 `ProjectKnowledgeService` 与既有项目索引，覆盖选择文档、来源状态、添加/重建/移除和打开原文。
- 索引内容和来源状态仍归 Application/Harness；原生视图只持有当前查询与选择。文档索引变化合并刷新，项目切换清除选择并拒绝迟到回复。
- 共用字段按窄窗口改为纵向排列；设置页结构与面包屑已逐项对照，其他设置缺口继续保留。
- 聚焦验证：`cargo test -p yss-application --lib harness::knowledge::tests::` 两项通过，覆盖正文变化/删除/移除与重开/跨项目隔离。
- `cargo check -p yss-desktop-gpui --bin yss-desktop-gpui` 和 `cargo clippy -p yss-desktop-gpui --bin yss-desktop-gpui --no-deps -- -D warnings` 通过。
- 本批界面人工验收保持开放，操作路径见 GPUI README；编译和业务测试不作为界面验收证据。

### 模型与供应商设置

- 已逐项阅读四个模型设置组件及关联目录/服务：供应商列表和默认模型、供应商编辑、可搜索选择器、模型编辑。
- 原生供应商选择复用 GPUI Combobox，直接使用服务预设；新增配置直接进入编辑，更换预设保留账户及自定义名称并清空模型和临时密钥。
- 已补齐独立协议选择、模型显示名回退与发现结果按 ID 自动合并；删除旧发现列表，保留已有参数和正在编辑的输入。
- 模型表单继续只挂载当前编辑项，复用共享类型校验；不复制 React 的每行输入状态或为未展开模型创建控件。
- 列表补齐实际供应商名称；列表和默认模型菜单借用目录，编辑按钮按稳定账户 ID 读取当前配置，去掉每次渲染的整份配置复制。
- 凭据归属由 Contract 定义，Application 在保存及发现时强制校验，避免更换供应商后隐式复用旧密钥；新加两项业务回归分别保护这两个入口。
- `cargo test -p yss-application --lib harness::models::tests::` 八项通过，覆盖发现草稿/过期回执、账户身份、密钥替换与配置持久化。
- `cargo clippy -p yss-desktop-gpui --bin yss-desktop-gpui --no-deps -- -D warnings` 与 `cargo clippy -p yss-application -p yss-harness-contract --lib --tests --no-deps -- -D warnings` 通过；UI 人工验收保持开放，具体路径见 GPUI README。

### 数据导入

- 已逐项阅读六个导入组件、导入步骤 hook、示例目录 hook 与 Application 调用编排，核对失败输入保留、重复提交、目录读取、表选择和完成后的资源打开。
- 原生 `ImportDialog` 使用统一步骤状态承接各子弹窗，复用原文件选择器、URL 输入和 Application 导入/发现服务；表选择继续使用虚拟列表。
- 补齐选择页来源标题：本地文件名可悬浮查看路径，远端只呈现引擎与服务器地址；不移植包含认证信息的原始连接串提示框。
- 示例页补齐本地化名称、用途说明、KB/MB 大小、加载/空目录/读取失败与重试、当前项进度。回调仅捕获 ID/版本，读取与导入错误不再统一丢弃分类。
- 连接草稿返回后保留自定义端口，切换引擎只更新原默认端口；脏输入判断使用当前引擎默认值。表单 Enter 复用当前步骤的提交入口与忙碌保护，窄窗口分类改为横向排列。
- 原生继续在显式确认或选择后提交，保留 CSV 参数与可选项目名称，不移植单表自动提交；GPUI 路径选择器不提供扩展名过滤，由所选入口的原 Reader 校验文件内容。
- React 的“其他 / REST API”只有禁用占位，没有导入服务，因此无需注册原生操作；实际支持来源继续保留 Parquet。
- `cargo test -p yss-application --lib database::tests::bundled_samples_import_edit_and_reopen_as_independent_project_datasets -- --exact` 一项通过，实际覆盖五个示例的独立导入、重复导入、编辑、保存和重开。
- `cargo check -p yss-desktop-gpui --bin yss-desktop-gpui` 与 `cargo clippy -p yss-desktop-gpui --bin yss-desktop-gpui --no-deps -- -D warnings` 通过；这批界面人工验收仍开放，具体操作见 GPUI README。

## app

| 参考文件（含其子组件） | 必要性/架构结论 | 原生对应与缺口 | 实现/验收 |
| --- | --- | --- | --- |
| [app/App.tsx](../../react/src/app/App.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [app/main.tsx](../../react/src/app/main.tsx) | 待查 | 待逐项阅读源码 | 待审查 |

## app/providers

| 参考文件（含其子组件） | 必要性/架构结论 | 原生对应与缺口 | 实现/验收 |
| --- | --- | --- | --- |
| [app/providers/ChartThemeProvider.tsx](../../react/src/app/providers/ChartThemeProvider.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [app/providers/SettingsEffectsProvider.tsx](../../react/src/app/providers/SettingsEffectsProvider.tsx) | 待查 | 待逐项阅读源码 | 待审查 |

## app/ui

| 参考文件（含其子组件） | 必要性/架构结论 | 原生对应与缺口 | 实现/验收 |
| --- | --- | --- | --- |
| [app/ui/UIHost.tsx](../../react/src/app/ui/UIHost.tsx) | 待查 | 待逐项阅读源码 | 待审查 |

## app/windows

| 参考文件（含其子组件） | 必要性/架构结论 | 原生对应与缺口 | 实现/验收 |
| --- | --- | --- | --- |
| [app/windows/workbench/WorkbenchComposition.tsx](../../react/src/app/windows/workbench/WorkbenchComposition.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [app/windows/workbench/integrations/PluginProvider.tsx](../../react/src/app/windows/workbench/integrations/PluginProvider.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [app/windows/workbench/integrations/activityEditorDndOverlay.tsx](../../react/src/app/windows/workbench/integrations/activityEditorDndOverlay.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [app/windows/workbench/menuContributionRegistry.tsx](../../react/src/app/windows/workbench/menuContributionRegistry.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [app/windows/workbench/rootPanelRegistry.tsx](../../react/src/app/windows/workbench/rootPanelRegistry.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [app/windows/workbench/rootPanelTabRenderer.tsx](../../react/src/app/windows/workbench/rootPanelTabRenderer.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [app/windows/workbench/statusBarContributionRegistry.tsx](../../react/src/app/windows/workbench/statusBarContributionRegistry.tsx) | 待查 | 待逐项阅读源码 | 待审查 |

## components/ui

| 参考文件（含其子组件） | 必要性/架构结论 | 原生对应与缺口 | 实现/验收 |
| --- | --- | --- | --- |
| [components/ui/alert.tsx](../../react/src/components/ui/alert.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [components/ui/badge.tsx](../../react/src/components/ui/badge.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [components/ui/button.tsx](../../react/src/components/ui/button.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [components/ui/card.tsx](../../react/src/components/ui/card.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [components/ui/checkbox.tsx](../../react/src/components/ui/checkbox.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [components/ui/collapsible.tsx](../../react/src/components/ui/collapsible.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [components/ui/combobox.tsx](../../react/src/components/ui/combobox.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [components/ui/context-menu.tsx](../../react/src/components/ui/context-menu.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [components/ui/dialog.tsx](../../react/src/components/ui/dialog.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [components/ui/dropdown-menu.tsx](../../react/src/components/ui/dropdown-menu.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [components/ui/empty.tsx](../../react/src/components/ui/empty.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [components/ui/input-group.tsx](../../react/src/components/ui/input-group.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [components/ui/input.tsx](../../react/src/components/ui/input.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [components/ui/label.tsx](../../react/src/components/ui/label.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [components/ui/menubar.tsx](../../react/src/components/ui/menubar.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [components/ui/popover.tsx](../../react/src/components/ui/popover.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [components/ui/progress.tsx](../../react/src/components/ui/progress.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [components/ui/scroll-area.tsx](../../react/src/components/ui/scroll-area.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [components/ui/select.tsx](../../react/src/components/ui/select.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [components/ui/separator.tsx](../../react/src/components/ui/separator.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [components/ui/switch.tsx](../../react/src/components/ui/switch.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [components/ui/table.tsx](../../react/src/components/ui/table.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [components/ui/textarea.tsx](../../react/src/components/ui/textarea.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [components/ui/toggle-group.tsx](../../react/src/components/ui/toggle-group.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [components/ui/toggle.tsx](../../react/src/components/ui/toggle.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [components/ui/tooltip.tsx](../../react/src/components/ui/tooltip.tsx) | 待查 | 待逐项阅读源码 | 待审查 |

## components/ui-presentation

| 参考文件（含其子组件） | 必要性/架构结论 | 原生对应与缺口 | 实现/验收 |
| --- | --- | --- | --- |
| [components/ui-presentation/Chart.tsx](../../react/src/components/ui-presentation/Chart.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [components/ui-presentation/CoefficientChart.tsx](../../react/src/components/ui-presentation/CoefficientChart.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [components/ui-presentation/CoefficientTable.tsx](../../react/src/components/ui-presentation/CoefficientTable.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [components/ui-presentation/Controls.tsx](../../react/src/components/ui-presentation/Controls.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [components/ui-presentation/DataTable.tsx](../../react/src/components/ui-presentation/DataTable.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [components/ui-presentation/Equation.tsx](../../react/src/components/ui-presentation/Equation.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [components/ui-presentation/FormulaMappingTable.tsx](../../react/src/components/ui-presentation/FormulaMappingTable.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [components/ui-presentation/KeyValue.tsx](../../react/src/components/ui-presentation/KeyValue.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [components/ui-presentation/Section.tsx](../../react/src/components/ui-presentation/Section.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [components/ui-presentation/StatCard.tsx](../../react/src/components/ui-presentation/StatCard.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [components/ui-presentation/StructuredData.tsx](../../react/src/components/ui-presentation/StructuredData.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [components/ui-presentation/TableFrame.tsx](../../react/src/components/ui-presentation/TableFrame.tsx) | 待查 | 待逐项阅读源码 | 待审查 |

## features/application

| 参考文件（含其子组件） | 必要性/架构结论 | 原生对应与缺口 | 实现/验收 |
| --- | --- | --- | --- |
| [features/application/assistant/AssistantRuntimeProvider.tsx](../../react/src/features/application/assistant/AssistantRuntimeProvider.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [features/application/presentation/LinePlotControls.tsx](../../react/src/features/application/presentation/LinePlotControls.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [features/application/presentation/PlotResultView.tsx](../../react/src/features/application/presentation/PlotResultView.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [features/application/results/components/ReadOnlyDataGrid.tsx](../../react/src/features/application/results/components/ReadOnlyDataGrid.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [features/application/results/components/ResultPageToolbar.tsx](../../react/src/features/application/results/components/ResultPageToolbar.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [features/application/results/components/ResultReadError.tsx](../../react/src/features/application/results/components/ResultReadError.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [features/application/results/components/ResultViewShell.tsx](../../react/src/features/application/results/components/ResultViewShell.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [features/application/results/components/UnifiedResultView.tsx](../../react/src/features/application/results/components/UnifiedResultView.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [features/application/results/components/renderers/ResultRenderers.tsx](../../react/src/features/application/results/components/renderers/ResultRenderers.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [features/application/results/resultViewPresentation.tsx](../../react/src/features/application/results/resultViewPresentation.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [features/application/statusBar/useStatusBarItems.tsx](../../react/src/features/application/statusBar/useStatusBarItems.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [features/application/window/PresentationWindowShell.tsx](../../react/src/features/application/window/PresentationWindowShell.tsx) | 待查 | 待逐项阅读源码 | 待审查 |

## features/core

| 参考文件（含其子组件） | 必要性/架构结论 | 原生对应与缺口 | 实现/验收 |
| --- | --- | --- | --- |
| [features/core/statusBar/builtInStatusBarItems.tsx](../../react/src/features/core/statusBar/builtInStatusBarItems.tsx) | 待查 | 待逐项阅读源码 | 待审查 |

## modules/assistant

| 参考文件（含其子组件） | 必要性/架构结论 | 原生对应与缺口 | 实现/验收 |
| --- | --- | --- | --- |
| [modules/assistant/internal/ui/AssistantComposer.tsx](../../react/src/modules/assistant/internal/ui/AssistantComposer.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/assistant/internal/ui/AssistantConversationHeader.tsx](../../react/src/modules/assistant/internal/ui/AssistantConversationHeader.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/assistant/internal/ui/AssistantConversationPanel.tsx](../../react/src/modules/assistant/internal/ui/AssistantConversationPanel.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/assistant/internal/ui/AssistantConversationToggle.tsx](../../react/src/modules/assistant/internal/ui/AssistantConversationToggle.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/assistant/internal/ui/AssistantConversations.tsx](../../react/src/modules/assistant/internal/ui/AssistantConversations.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/assistant/internal/ui/AssistantExecution.tsx](../../react/src/modules/assistant/internal/ui/AssistantExecution.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/assistant/internal/ui/AssistantMarkdown.tsx](../../react/src/modules/assistant/internal/ui/AssistantMarkdown.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/assistant/internal/ui/AssistantModelPicker.tsx](../../react/src/modules/assistant/internal/ui/AssistantModelPicker.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/assistant/internal/ui/AssistantPanel.tsx](../../react/src/modules/assistant/internal/ui/AssistantPanel.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/assistant/internal/ui/AssistantReferences.tsx](../../react/src/modules/assistant/internal/ui/AssistantReferences.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/assistant/internal/ui/AssistantResources.tsx](../../react/src/modules/assistant/internal/ui/AssistantResources.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/assistant/internal/ui/AssistantRunOptions.tsx](../../react/src/modules/assistant/internal/ui/AssistantRunOptions.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/assistant/internal/ui/AssistantTasks.tsx](../../react/src/modules/assistant/internal/ui/AssistantTasks.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/assistant/internal/ui/AssistantThread.tsx](../../react/src/modules/assistant/internal/ui/AssistantThread.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/assistant/internal/ui/AssistantTokenUsage.tsx](../../react/src/modules/assistant/internal/ui/AssistantTokenUsage.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/assistant/internal/ui/AssistantToolCalls.tsx](../../react/src/modules/assistant/internal/ui/AssistantToolCalls.tsx) | 待查 | 待逐项阅读源码 | 待审查 |

## modules/chart

| 参考文件（含其子组件） | 必要性/架构结论 | 原生对应与缺口 | 实现/验收 |
| --- | --- | --- | --- |
| [modules/chart/internal/ui/ChartEditor.tsx](../../react/src/modules/chart/internal/ui/ChartEditor.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/chart/internal/ui/ChartEmptyState.tsx](../../react/src/modules/chart/internal/ui/ChartEmptyState.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/chart/internal/ui/ChartPreview.tsx](../../react/src/modules/chart/internal/ui/ChartPreview.tsx) | 待查 | 待逐项阅读源码 | 待审查 |

## modules/commands

| 参考文件（含其子组件） | 必要性/架构结论 | 原生对应与缺口 | 实现/验收 |
| --- | --- | --- | --- |
| [modules/commands/internal/ui/activity/SidebarCommandsTab.tsx](../../react/src/modules/commands/internal/ui/activity/SidebarCommandsTab.tsx) | 待查 | 待逐项阅读源码 | 待审查 |

## modules/data-explorer

| 参考文件（含其子组件） | 必要性/架构结论 | 原生对应与缺口 | 实现/验收 |
| --- | --- | --- | --- |
| [modules/data-explorer/internal/ui/import/ExcelSheetSelectModal.tsx](../../react/src/modules/data-explorer/internal/ui/import/ExcelSheetSelectModal.tsx) | 优化：工作表选择复用原生统一步骤和虚拟列表，无需另建弹窗状态 | `imports/selection` 补齐文件名与完整路径提示；失败保留工作表列表与输入，返回继续原流程 | 代码已覆盖；人工验收待完成 |
| [modules/data-explorer/internal/ui/import/ImportModal.tsx](../../react/src/modules/data-explorer/internal/ui/import/ImportModal.tsx) | 迁移/优化：原生分类、步骤和窗口复用原服务；禁用的 REST API 占位无需迁移 | 三类来源、取消/忙碌保护与失败保留已接入；补齐窄窗口分类；保留显式确认及原生路径选择器，完成后按实际回执打开数据 | 代码已覆盖；人工验收待完成 |
| [modules/data-explorer/internal/ui/import/SampleDatasetList.tsx](../../react/src/modules/data-explorer/internal/ui/import/SampleDatasetList.tsx) | 迁移：目录与导入继续由 Application 持有，视图仅持有读投影和当前任务 | `imports/samples` 补齐名称/说明、尺寸单位、空/加载/失败/重试和当前项进度；`feedback` 保留类型化错误分类 | 示例业务回归通过；人工验收待完成 |
| [modules/data-explorer/internal/ui/import/SqlConnectionModal.tsx](../../react/src/modules/data-explorer/internal/ui/import/SqlConnectionModal.tsx) | 优化：复用原生输入和 URL 编码，连接草稿留在统一流程，不新增连接存储 | `imports/inputs` 保留字段/遮蔽连接串模式与失败草稿；修复返回后的端口重置和默认端口脏判断 | 代码已覆盖；人工验收待完成 |
| [modules/data-explorer/internal/ui/import/SqlRemoteTableSelectModal.tsx](../../react/src/modules/data-explorer/internal/ui/import/SqlRemoteTableSelectModal.tsx) | 优化：与 Excel/SQLite 共用虚拟选择列表，连接读取沿用原 Application | `imports/selection` 补齐服务器标识；提示框不包含认证或查询参数；失败后保留表选择与连接草稿 | 代码已覆盖；人工验收待完成 |
| [modules/data-explorer/internal/ui/import/SqliteTableSelectModal.tsx](../../react/src/modules/data-explorer/internal/ui/import/SqliteTableSelectModal.tsx) | 优化：复用同一来源选择器和原 SQLite 发现/导入入口 | `imports/selection` 补齐数据库文件名/路径提示，保留显式选择、返回和失败输入 | 代码已覆盖；人工验收待完成 |

## modules/database-editor

| 参考文件（含其子组件） | 必要性/架构结论 | 原生对应与缺口 | 实现/验收 |
| --- | --- | --- | --- |
| [modules/database-editor/internal/ui/DatabaseEditorContent.tsx](../../react/src/modules/database-editor/internal/ui/DatabaseEditorContent.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/database-editor/internal/ui/Layout/Toolbar.tsx](../../react/src/modules/database-editor/internal/ui/Layout/Toolbar.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/database-editor/internal/ui/Table/DataTable.tsx](../../react/src/modules/database-editor/internal/ui/Table/DataTable.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/database-editor/internal/ui/Table/DatabaseGridRenderers.tsx](../../react/src/modules/database-editor/internal/ui/Table/DatabaseGridRenderers.tsx) | 待查 | 待逐项阅读源码 | 待审查 |

## modules/details

| 参考文件（含其子组件） | 必要性/架构结论 | 原生对应与缺口 | 实现/验收 |
| --- | --- | --- | --- |
| [modules/details/internal/ui/DetailEmptyState.tsx](../../react/src/modules/details/internal/ui/DetailEmptyState.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/details/internal/ui/DetailsPane.tsx](../../react/src/modules/details/internal/ui/DetailsPane.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/details/internal/ui/constantValue/ConstantValueEditorModal.tsx](../../react/src/modules/details/internal/ui/constantValue/ConstantValueEditorModal.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/details/internal/ui/node/DescriptionResultSection.tsx](../../react/src/modules/details/internal/ui/node/DescriptionResultSection.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/details/internal/ui/node/NodeCreationForm.tsx](../../react/src/modules/details/internal/ui/node/NodeCreationForm.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/details/internal/ui/node/NodeDocumentationPanel.tsx](../../react/src/modules/details/internal/ui/node/NodeDocumentationPanel.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/details/internal/ui/node/NodePinConnectionField.tsx](../../react/src/modules/details/internal/ui/node/NodePinConnectionField.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/details/internal/ui/node/NodePinInterfacePanel.tsx](../../react/src/modules/details/internal/ui/node/NodePinInterfacePanel.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/details/internal/ui/node/NodePortInstanceControls.tsx](../../react/src/modules/details/internal/ui/node/NodePortInstanceControls.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/details/internal/ui/node/parameterEditors/NodeParameterEditor.tsx](../../react/src/modules/details/internal/ui/node/parameterEditors/NodeParameterEditor.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/details/internal/ui/node/parameterEditors/RelationalParameterEditors.tsx](../../react/src/modules/details/internal/ui/node/parameterEditors/RelationalParameterEditors.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/details/internal/ui/node/parameterEditors/SemanticDomainEditor.tsx](../../react/src/modules/details/internal/ui/node/parameterEditors/SemanticDomainEditor.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/details/internal/ui/panels/ChartDetailPanel.tsx](../../react/src/modules/details/internal/ui/panels/ChartDetailPanel.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/details/internal/ui/panels/ConstantValueFields.tsx](../../react/src/modules/details/internal/ui/panels/ConstantValueFields.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/details/internal/ui/panels/DataColumnSemanticDialog.tsx](../../react/src/modules/details/internal/ui/panels/DataColumnSemanticDialog.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/details/internal/ui/panels/DataColumnSemanticFields.tsx](../../react/src/modules/details/internal/ui/panels/DataColumnSemanticFields.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/details/internal/ui/panels/DataColumnSettings.tsx](../../react/src/modules/details/internal/ui/panels/DataColumnSettings.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/details/internal/ui/panels/DataDetailPanel.tsx](../../react/src/modules/details/internal/ui/panels/DataDetailPanel.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/details/internal/ui/panels/DataSelectionPreview.tsx](../../react/src/modules/details/internal/ui/panels/DataSelectionPreview.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/details/internal/ui/panels/EventDetailPanel.tsx](../../react/src/modules/details/internal/ui/panels/EventDetailPanel.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/details/internal/ui/panels/FileDetailPanel.tsx](../../react/src/modules/details/internal/ui/panels/FileDetailPanel.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/details/internal/ui/panels/FunctionDetailPanel.tsx](../../react/src/modules/details/internal/ui/panels/FunctionDetailPanel.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/details/internal/ui/panels/GraphConstantsPanel.tsx](../../react/src/modules/details/internal/ui/panels/GraphConstantsPanel.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/details/internal/ui/panels/LogDetailPanel.tsx](../../react/src/modules/details/internal/ui/panels/LogDetailPanel.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/details/internal/ui/panels/MindDetailPanel.tsx](../../react/src/modules/details/internal/ui/panels/MindDetailPanel.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/details/internal/ui/panels/NodeDefinitionDetailPanel.tsx](../../react/src/modules/details/internal/ui/panels/NodeDefinitionDetailPanel.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/details/internal/ui/panels/NodeDetailPanel.tsx](../../react/src/modules/details/internal/ui/panels/NodeDetailPanel.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/details/internal/ui/shared/DetailCollapsibleSection.tsx](../../react/src/modules/details/internal/ui/shared/DetailCollapsibleSection.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/details/internal/ui/shared/DetailColumnList.tsx](../../react/src/modules/details/internal/ui/shared/DetailColumnList.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/details/internal/ui/shared/DetailFieldRow.tsx](../../react/src/modules/details/internal/ui/shared/DetailFieldRow.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/details/internal/ui/shared/DetailForm.tsx](../../react/src/modules/details/internal/ui/shared/DetailForm.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/details/internal/ui/shared/DetailPanelShell.tsx](../../react/src/modules/details/internal/ui/shared/DetailPanelShell.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/details/internal/ui/shared/DetailText.tsx](../../react/src/modules/details/internal/ui/shared/DetailText.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/details/internal/ui/shared/PinEditor.tsx](../../react/src/modules/details/internal/ui/shared/PinEditor.tsx) | 待查 | 待逐项阅读源码 | 待审查 |

## modules/document-editor

| 参考文件（含其子组件） | 必要性/架构结论 | 原生对应与缺口 | 实现/验收 |
| --- | --- | --- | --- |
| [modules/document-editor/internal/DocEditor.tsx](../../react/src/modules/document-editor/internal/DocEditor.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/document-editor/internal/FileEditor.tsx](../../react/src/modules/document-editor/internal/FileEditor.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/document-editor/internal/MindEditor.tsx](../../react/src/modules/document-editor/internal/MindEditor.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/document-editor/internal/ReferencePanel.tsx](../../react/src/modules/document-editor/internal/ReferencePanel.tsx) | 待查 | 待逐项阅读源码 | 待审查 |

## modules/graph-editor

| 参考文件（含其子组件） | 必要性/架构结论 | 原生对应与缺口 | 实现/验收 |
| --- | --- | --- | --- |
| [modules/graph-editor/internal/ui/Canvas/core/CanvasDropZone.tsx](../../react/src/modules/graph-editor/internal/ui/Canvas/core/CanvasDropZone.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/graph-editor/internal/ui/Canvas/core/Edge.tsx](../../react/src/modules/graph-editor/internal/ui/Canvas/core/Edge.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/graph-editor/internal/ui/Canvas/core/GraphCanvasController.tsx](../../react/src/modules/graph-editor/internal/ui/Canvas/core/GraphCanvasController.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/graph-editor/internal/ui/Canvas/core/GraphCanvasView.tsx](../../react/src/modules/graph-editor/internal/ui/Canvas/core/GraphCanvasView.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/graph-editor/internal/ui/Canvas/core/GraphDocumentEditor.tsx](../../react/src/modules/graph-editor/internal/ui/Canvas/core/GraphDocumentEditor.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/graph-editor/internal/ui/Canvas/core/GraphFlowCanvas.tsx](../../react/src/modules/graph-editor/internal/ui/Canvas/core/GraphFlowCanvas.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/graph-editor/internal/ui/Canvas/core/GraphFlowConnection.tsx](../../react/src/modules/graph-editor/internal/ui/Canvas/core/GraphFlowConnection.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/graph-editor/internal/ui/Canvas/core/GraphFlowEdge.tsx](../../react/src/modules/graph-editor/internal/ui/Canvas/core/GraphFlowEdge.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/graph-editor/internal/ui/Canvas/core/GraphFlowNode.tsx](../../react/src/modules/graph-editor/internal/ui/Canvas/core/GraphFlowNode.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/graph-editor/internal/ui/Canvas/core/ViewportGrid.tsx](../../react/src/modules/graph-editor/internal/ui/Canvas/core/ViewportGrid.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/graph-editor/internal/ui/Canvas/overlays/CanvasExecutionToolbar.tsx](../../react/src/modules/graph-editor/internal/ui/Canvas/overlays/CanvasExecutionToolbar.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/graph-editor/internal/ui/Canvas/overlays/CanvasOverlays.tsx](../../react/src/modules/graph-editor/internal/ui/Canvas/overlays/CanvasOverlays.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/graph-editor/internal/ui/Canvas/overlays/PinResultSearchPalette.tsx](../../react/src/modules/graph-editor/internal/ui/Canvas/overlays/PinResultSearchPalette.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/graph-editor/internal/ui/Canvas/overlays/WatermarkView.tsx](../../react/src/modules/graph-editor/internal/ui/Canvas/overlays/WatermarkView.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/graph-editor/internal/ui/ContextMenu/ConnectionContextMenu.tsx](../../react/src/modules/graph-editor/internal/ui/ContextMenu/ConnectionContextMenu.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/graph-editor/internal/ui/ContextMenu/NodeContextMenu.tsx](../../react/src/modules/graph-editor/internal/ui/ContextMenu/NodeContextMenu.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/graph-editor/internal/ui/ContextMenu/PinContextMenu.tsx](../../react/src/modules/graph-editor/internal/ui/ContextMenu/PinContextMenu.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/graph-editor/internal/ui/NodePalette.tsx](../../react/src/modules/graph-editor/internal/ui/NodePalette.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/graph-editor/internal/ui/Nodes/DefaultNodeLayout.tsx](../../react/src/modules/graph-editor/internal/ui/Nodes/DefaultNodeLayout.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/graph-editor/internal/ui/Nodes/GraphNodeController.tsx](../../react/src/modules/graph-editor/internal/ui/Nodes/GraphNodeController.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/graph-editor/internal/ui/Nodes/GraphNodeView.tsx](../../react/src/modules/graph-editor/internal/ui/Nodes/GraphNodeView.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/graph-editor/internal/ui/Nodes/RerouteNodeLayout.tsx](../../react/src/modules/graph-editor/internal/ui/Nodes/RerouteNodeLayout.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/graph-editor/internal/ui/Pins/GraphPinController.tsx](../../react/src/modules/graph-editor/internal/ui/Pins/GraphPinController.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/graph-editor/internal/ui/Pins/GraphPinView.tsx](../../react/src/modules/graph-editor/internal/ui/Pins/GraphPinView.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/graph-editor/internal/ui/Pins/PinInput.tsx](../../react/src/modules/graph-editor/internal/ui/Pins/PinInput.tsx) | 待查 | 待逐项阅读源码 | 待审查 |

## modules/logs

| 参考文件（含其子组件） | 必要性/架构结论 | 原生对应与缺口 | 实现/验收 |
| --- | --- | --- | --- |
| [modules/logs/internal/ui/LogDomainLayoutHost.tsx](../../react/src/modules/logs/internal/ui/LogDomainLayoutHost.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/logs/internal/ui/LogDomainPanel.tsx](../../react/src/modules/logs/internal/ui/LogDomainPanel.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/logs/internal/ui/LogItemRow.tsx](../../react/src/modules/logs/internal/ui/LogItemRow.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/logs/internal/ui/LogPanelList.tsx](../../react/src/modules/logs/internal/ui/LogPanelList.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/logs/internal/ui/LogPanelStatus.tsx](../../react/src/modules/logs/internal/ui/LogPanelStatus.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/logs/internal/ui/LogPanelToolbar.tsx](../../react/src/modules/logs/internal/ui/LogPanelToolbar.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/logs/internal/ui/LogPanelVirtualList.tsx](../../react/src/modules/logs/internal/ui/LogPanelVirtualList.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/logs/internal/ui/LogWindow.tsx](../../react/src/modules/logs/internal/ui/LogWindow.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/logs/internal/ui/LogWorkspaceActions.tsx](../../react/src/modules/logs/internal/ui/LogWorkspaceActions.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/logs/internal/ui/logWorkspaceContext.tsx](../../react/src/modules/logs/internal/ui/logWorkspaceContext.tsx) | 待查 | 待逐项阅读源码 | 待审查 |

## modules/node-catalog

| 参考文件（含其子组件） | 必要性/架构结论 | 原生对应与缺口 | 实现/验收 |
| --- | --- | --- | --- |
| [modules/node-catalog/internal/ui/activity/LocalizedCatalogTreeRow.tsx](../../react/src/modules/node-catalog/internal/ui/activity/LocalizedCatalogTreeRow.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/node-catalog/internal/ui/activity/SidebarNodesTab.tsx](../../react/src/modules/node-catalog/internal/ui/activity/SidebarNodesTab.tsx) | 待查 | 待逐项阅读源码 | 待审查 |

## modules/output

| 参考文件（含其子组件） | 必要性/架构结论 | 原生对应与缺口 | 实现/验收 |
| --- | --- | --- | --- |
| [modules/output/internal/ui/RunFailurePanel.tsx](../../react/src/modules/output/internal/ui/RunFailurePanel.tsx) | 待查 | 待逐项阅读源码 | 待审查 |

## modules/plugins

| 参考文件（含其子组件） | 必要性/架构结论 | 原生对应与缺口 | 实现/验收 |
| --- | --- | --- | --- |
| [modules/plugins/internal/ui/PluginMaintenanceDialog.tsx](../../react/src/modules/plugins/internal/ui/PluginMaintenanceDialog.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/plugins/internal/ui/PluginViewFrame.tsx](../../react/src/modules/plugins/internal/ui/PluginViewFrame.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/plugins/internal/ui/PluginsPanel.tsx](../../react/src/modules/plugins/internal/ui/PluginsPanel.tsx) | 待查 | 待逐项阅读源码 | 待审查 |

## modules/problems

| 参考文件（含其子组件） | 必要性/架构结论 | 原生对应与缺口 | 实现/验收 |
| --- | --- | --- | --- |
| [modules/problems/internal/ui/GraphProblemsPanel.tsx](../../react/src/modules/problems/internal/ui/GraphProblemsPanel.tsx) | 待查 | 待逐项阅读源码 | 待审查 |

## modules/project-explorer

| 参考文件（含其子组件） | 必要性/架构结论 | 原生对应与缺口 | 实现/验收 |
| --- | --- | --- | --- |
| [modules/project-explorer/internal/ui/activity/ProjectActivityPanelController.tsx](../../react/src/modules/project-explorer/internal/ui/activity/ProjectActivityPanelController.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/project-explorer/internal/ui/activity/SidebarDataRow.tsx](../../react/src/modules/project-explorer/internal/ui/activity/SidebarDataRow.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/project-explorer/internal/ui/activity/SidebarFileRow.tsx](../../react/src/modules/project-explorer/internal/ui/activity/SidebarFileRow.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/project-explorer/internal/ui/activity/SidebarProjectTab.tsx](../../react/src/modules/project-explorer/internal/ui/activity/SidebarProjectTab.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/project-explorer/internal/ui/activity/SidebarProjectTreeRow.tsx](../../react/src/modules/project-explorer/internal/ui/activity/SidebarProjectTreeRow.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/project-explorer/internal/ui/activity/buildProjectSidebarContextMenuSections.tsx](../../react/src/modules/project-explorer/internal/ui/activity/buildProjectSidebarContextMenuSections.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/project-explorer/internal/ui/picker/DeleteProjectConfirmDialog.tsx](../../react/src/modules/project-explorer/internal/ui/picker/DeleteProjectConfirmDialog.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/project-explorer/internal/ui/picker/NewProjectModal.tsx](../../react/src/modules/project-explorer/internal/ui/picker/NewProjectModal.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/project-explorer/internal/ui/picker/ProjectLibrary.tsx](../../react/src/modules/project-explorer/internal/ui/picker/ProjectLibrary.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/project-explorer/internal/ui/picker/ProjectPickerActionPanel.tsx](../../react/src/modules/project-explorer/internal/ui/picker/ProjectPickerActionPanel.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/project-explorer/internal/ui/picker/ProjectPickerChrome.tsx](../../react/src/modules/project-explorer/internal/ui/picker/ProjectPickerChrome.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/project-explorer/internal/ui/picker/ProjectPickerFeedbackDetails.tsx](../../react/src/modules/project-explorer/internal/ui/picker/ProjectPickerFeedbackDetails.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/project-explorer/internal/ui/picker/ProjectPickerPageIssueAlert.tsx](../../react/src/modules/project-explorer/internal/ui/picker/ProjectPickerPageIssueAlert.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/project-explorer/internal/ui/picker/ProjectPickerScreen.tsx](../../react/src/modules/project-explorer/internal/ui/picker/ProjectPickerScreen.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/project-explorer/internal/ui/picker/projectPickerContextMenu/buildProjectPickerContextMenuSections.tsx](../../react/src/modules/project-explorer/internal/ui/picker/projectPickerContextMenu/buildProjectPickerContextMenuSections.tsx) | 待查 | 待逐项阅读源码 | 待审查 |

## modules/results

| 参考文件（含其子组件） | 必要性/架构结论 | 原生对应与缺口 | 实现/验收 |
| --- | --- | --- | --- |
| [modules/results/internal/ui/info/AddReportContents.tsx](../../react/src/modules/results/internal/ui/info/AddReportContents.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/results/internal/ui/info/LinearRegressionReport.tsx](../../react/src/modules/results/internal/ui/info/LinearRegressionReport.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/results/internal/ui/info/ReportView.tsx](../../react/src/modules/results/internal/ui/info/ReportView.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/results/internal/ui/info/StructuredReportTable.tsx](../../react/src/modules/results/internal/ui/info/StructuredReportTable.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/results/internal/ui/info/StructuredResult.tsx](../../react/src/modules/results/internal/ui/info/StructuredResult.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/results/internal/ui/info/shared/ACFPACFBlock.tsx](../../react/src/modules/results/internal/ui/info/shared/ACFPACFBlock.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/results/internal/ui/info/shared/HypothesisTestBlock.tsx](../../react/src/modules/results/internal/ui/info/shared/HypothesisTestBlock.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/results/internal/ui/info/shared/SerialTestsBlock.tsx](../../react/src/modules/results/internal/ui/info/shared/SerialTestsBlock.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/results/internal/ui/panel/ResultContent.tsx](../../react/src/modules/results/internal/ui/panel/ResultContent.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/results/internal/ui/panel/ResultInspector.tsx](../../react/src/modules/results/internal/ui/panel/ResultInspector.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/results/internal/ui/panel/ResultPanel.tsx](../../react/src/modules/results/internal/ui/panel/ResultPanel.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/results/internal/ui/plot/PlotWindow.tsx](../../react/src/modules/results/internal/ui/plot/PlotWindow.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/results/internal/ui/source-inspector/SourceInspectorWindow.tsx](../../react/src/modules/results/internal/ui/source-inspector/SourceInspectorWindow.tsx) | 待查 | 待逐项阅读源码 | 待审查 |

## modules/settings

| 参考文件（含其子组件） | 必要性/架构结论 | 原生对应与缺口 | 实现/验收 |
| --- | --- | --- | --- |
| [modules/settings/internal/ui/KnowledgeSettings.tsx](../../react/src/modules/settings/internal/ui/KnowledgeSettings.tsx) | 迁移：保留显式项目文档索引管理；原生直接调用 Application，移除 Web IPC 适配需求 | `settings/knowledge` 状态/命令/渲染；`workbench/settings` 注入项目与打开原文；来源刷新按文档变化合并 | 已实现并通过聚焦业务测试/编译/Clippy；人工验收待完成 |
| [modules/settings/internal/ui/LanguageModelEditor.tsx](../../react/src/modules/settings/internal/ui/LanguageModelEditor.tsx) | 优化：只挂载当前编辑模型，数值/JSON/推理配置继续复用共享校验 | `settings/models` 已有单模型输入及应用/取消，补齐空显示名使用模型 ID；未编辑项只保留原配置 | 代码已覆盖；人工验收待完成 |
| [modules/settings/internal/ui/LanguageModelProviderEditor.tsx](../../react/src/modules/settings/internal/ui/LanguageModelProviderEditor.tsx) | 迁移/优化：连接编辑和模型草稿留在 UI，保存/发现及凭据归属由 Application 负责 | `settings/models/provider` 补齐预设切换、独立协议/认证与换密钥提示；`commands` 合并发现结果并保留原参数 | 业务回归通过；人工验收待完成 |
| [modules/settings/internal/ui/LanguageModelProviderSelect.tsx](../../react/src/modules/settings/internal/ui/LanguageModelProviderSelect.tsx) | 复用原生组件：GPUI Combobox 替代 shadcn/Base UI，搜索与键盘由组件处理 | 直接消费 Application 预设，重选同项不重置，切换保留稳定账户 ID 与自定义名称 | 代码已覆盖；人工验收待完成 |
| [modules/settings/internal/ui/LanguageModelSettings.tsx](../../react/src/modules/settings/internal/ui/LanguageModelSettings.tsx) | 优化：目录仍由 Application 持有，原生仅缓存读投影和未提交草稿 | 默认模型、列表、删除与存储已接入；新增直接打开编辑器，面包屑统一使用配置显示名 | 代码已覆盖；人工验收待完成 |
| [modules/settings/internal/ui/SettingsField.tsx](../../react/src/modules/settings/internal/ui/SettingsField.tsx) | 优化：共用名称/说明/控件行，不迁移 DOM Label 包装 | `settings/fields` 按窗口宽度排列，模型与知识库共用；键盘/焦点与标签关系须人工检查 | 实现已复核；人工验收待完成 |
| [modules/settings/internal/ui/SettingsPage.tsx](../../react/src/modules/settings/internal/ui/SettingsPage.tsx) | 复用原生组件：标题/动作/通知固定，正文独立滚动 | `settings/render` 统一页面结构；知识库失败/加载提示在正文滚动区之外 | 已有原生实现；人工验收待完成 |
| [modules/settings/internal/ui/SettingsPageHeader.tsx](../../react/src/modules/settings/internal/ui/SettingsPageHeader.tsx) | 复用原生组件：原生按钮/图标展示面包屑与页面操作 | `settings/render::header`；模型草稿导航保留放弃确认，知识库使用独立标题；长标题提示和焦点待检查 | 已有原生实现；人工验收待完成 |
| [modules/settings/internal/ui/SettingsView.tsx](../../react/src/modules/settings/internal/ui/SettingsView.tsx) | 迁移/复核：原生独立设置窗口替代 React Dialog，保留分类、搜索与外观设置 | 知识库已接入；语言、搜索、主题、标题栏、平滑滚动及重置仍需逐项复核/补齐 | 审查中，不以知识库完成代表整个设置完成 |

## modules/workbench

| 参考文件（含其子组件） | 必要性/架构结论 | 原生对应与缺口 | 实现/验收 |
| --- | --- | --- | --- |
| [modules/workbench/internal/layout/EditorResourcePanel.tsx](../../react/src/modules/workbench/internal/layout/EditorResourcePanel.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/workbench/internal/layout/RootLayoutHost.tsx](../../react/src/modules/workbench/internal/layout/RootLayoutHost.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/workbench/internal/layout/RootPanelTabRenderer.tsx](../../react/src/modules/workbench/internal/layout/RootPanelTabRenderer.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/workbench/internal/ui/WorkbenchWindow.tsx](../../react/src/modules/workbench/internal/ui/WorkbenchWindow.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/workbench/internal/ui/WorkbenchWindowEntry.tsx](../../react/src/modules/workbench/internal/ui/WorkbenchWindowEntry.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/workbench/internal/ui/activity/ActivityPanelDocumentView.tsx](../../react/src/modules/workbench/internal/ui/activity/ActivityPanelDocumentView.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/workbench/internal/ui/activity/ActivityPanelShell.tsx](../../react/src/modules/workbench/internal/ui/activity/ActivityPanelShell.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/workbench/internal/ui/dnd/SidebarDragOverlay.tsx](../../react/src/modules/workbench/internal/ui/dnd/SidebarDragOverlay.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/workbench/internal/ui/menu/AboutModal.tsx](../../react/src/modules/workbench/internal/ui/menu/AboutModal.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/workbench/internal/ui/menu/ArchitectureModal.tsx](../../react/src/modules/workbench/internal/ui/menu/ArchitectureModal.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/workbench/internal/ui/menu/BackendArchitecture.tsx](../../react/src/modules/workbench/internal/ui/menu/BackendArchitecture.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/workbench/internal/ui/menu/CommunicationArchitecture.tsx](../../react/src/modules/workbench/internal/ui/menu/CommunicationArchitecture.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/workbench/internal/ui/menu/CrateDependencies.tsx](../../react/src/modules/workbench/internal/ui/menu/CrateDependencies.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/workbench/internal/ui/menu/FrontendArchitecture.tsx](../../react/src/modules/workbench/internal/ui/menu/FrontendArchitecture.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/workbench/internal/ui/menu/WorkbenchMenuBar.tsx](../../react/src/modules/workbench/internal/ui/menu/WorkbenchMenuBar.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/workbench/internal/ui/sidebar/SidebarEmptyState.tsx](../../react/src/modules/workbench/internal/ui/sidebar/SidebarEmptyState.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/workbench/internal/ui/sidebar/SidebarRenameDialog.tsx](../../react/src/modules/workbench/internal/ui/sidebar/SidebarRenameDialog.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/workbench/internal/ui/sidebar/SidebarSectionEmptyState.tsx](../../react/src/modules/workbench/internal/ui/sidebar/SidebarSectionEmptyState.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/workbench/internal/ui/sidebar/primitives/SidebarChevron.tsx](../../react/src/modules/workbench/internal/ui/sidebar/primitives/SidebarChevron.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/workbench/internal/ui/sidebar/primitives/SidebarDraggableItem.tsx](../../react/src/modules/workbench/internal/ui/sidebar/primitives/SidebarDraggableItem.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/workbench/internal/ui/sidebar/primitives/SidebarListItem.tsx](../../react/src/modules/workbench/internal/ui/sidebar/primitives/SidebarListItem.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/workbench/internal/ui/sidebar/primitives/SidebarRowActionButton.tsx](../../react/src/modules/workbench/internal/ui/sidebar/primitives/SidebarRowActionButton.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/workbench/internal/ui/sidebar/primitives/SidebarTreeCategoryRow.tsx](../../react/src/modules/workbench/internal/ui/sidebar/primitives/SidebarTreeCategoryRow.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/workbench/internal/ui/sidebar/primitives/SidebarTreeSearchInput.tsx](../../react/src/modules/workbench/internal/ui/sidebar/primitives/SidebarTreeSearchInput.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/workbench/internal/ui/status/StatusBar.tsx](../../react/src/modules/workbench/internal/ui/status/StatusBar.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/workbench/internal/ui/status/StatusBarItem.tsx](../../react/src/modules/workbench/internal/ui/status/StatusBarItem.tsx) | 待查 | 待逐项阅读源码 | 待审查 |

## shared/charts

| 参考文件（含其子组件） | 必要性/架构结论 | 原生对应与缺口 | 实现/验收 |
| --- | --- | --- | --- |
| [shared/charts/ChartRenderer.tsx](../../react/src/shared/charts/ChartRenderer.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [shared/charts/cartesian/CompositeChart.tsx](../../react/src/shared/charts/cartesian/CompositeChart.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [shared/charts/cartesian/EcdfChart.tsx](../../react/src/shared/charts/cartesian/EcdfChart.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [shared/charts/cartesian/HistogramChart.tsx](../../react/src/shared/charts/cartesian/HistogramChart.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [shared/charts/cartesian/KdeChart.tsx](../../react/src/shared/charts/cartesian/KdeChart.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [shared/charts/cartesian/LineChart.tsx](../../react/src/shared/charts/cartesian/LineChart.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [shared/charts/cartesian/ScatterChart.tsx](../../react/src/shared/charts/cartesian/ScatterChart.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [shared/charts/categorical/WordCloudChart.tsx](../../react/src/shared/charts/categorical/WordCloudChart.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [shared/charts/core/theme.tsx](../../react/src/shared/charts/core/theme.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [shared/charts/statistical/CorrelationMatrixChart.tsx](../../react/src/shared/charts/statistical/CorrelationMatrixChart.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [shared/charts/statistical/CorrelogramChart.tsx](../../react/src/shared/charts/statistical/CorrelogramChart.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [shared/charts/statistical/DistributionChart.tsx](../../react/src/shared/charts/statistical/DistributionChart.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [shared/charts/statistical/HeatmapChart.tsx](../../react/src/shared/charts/statistical/HeatmapChart.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [shared/charts/statistical/IntervalChart.tsx](../../react/src/shared/charts/statistical/IntervalChart.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [shared/charts/statistical/NomogramChart.tsx](../../react/src/shared/charts/statistical/NomogramChart.tsx) | 待查 | 待逐项阅读源码 | 待审查 |

## shared/ui

| 参考文件（含其子组件） | 必要性/架构结论 | 原生对应与缺口 | 实现/验收 |
| --- | --- | --- | --- |
| [shared/ui/BrandMark.tsx](../../react/src/shared/ui/BrandMark.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [shared/ui/MarkdownLink.tsx](../../react/src/shared/ui/MarkdownLink.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [shared/ui/MarkdownRenderer.tsx](../../react/src/shared/ui/MarkdownRenderer.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [shared/ui/MessageDialog.tsx](../../react/src/shared/ui/MessageDialog.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [shared/ui/Modal.tsx](../../react/src/shared/ui/Modal.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [shared/ui/PageAlert.tsx](../../react/src/shared/ui/PageAlert.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [shared/ui/ProgressOverlay.tsx](../../react/src/shared/ui/ProgressOverlay.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [shared/ui/Select.tsx](../../react/src/shared/ui/Select.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [shared/ui/ToolbarIconButton.tsx](../../react/src/shared/ui/ToolbarIconButton.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [shared/ui/WindowChrome.tsx](../../react/src/shared/ui/WindowChrome.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [shared/ui/WindowChromeControls.tsx](../../react/src/shared/ui/WindowChromeControls.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [shared/ui/WindowTitleBar.tsx](../../react/src/shared/ui/WindowTitleBar.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [shared/ui/actionMenu/ActionMenu.tsx](../../react/src/shared/ui/actionMenu/ActionMenu.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [shared/ui/markdownRendering.tsx](../../react/src/shared/ui/markdownRendering.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
