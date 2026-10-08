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

## 当前批次

- 项目知识库设置：React `KnowledgeSettings` 有项目文档选择、来源状态、添加/重建/移除和打开原文；原生设置仅实现模型配置。
- 复用 `ProjectKnowledgeService` 与既有项目索引，索引内容和来源状态仍归 Application/Harness；原生视图只持有当前查询与选择。
- 界面未保存模型草稿、项目切换和设置独立窗口生命周期须继续保持。

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
| [modules/data-explorer/internal/ui/import/ExcelSheetSelectModal.tsx](../../react/src/modules/data-explorer/internal/ui/import/ExcelSheetSelectModal.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/data-explorer/internal/ui/import/ImportModal.tsx](../../react/src/modules/data-explorer/internal/ui/import/ImportModal.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/data-explorer/internal/ui/import/SampleDatasetList.tsx](../../react/src/modules/data-explorer/internal/ui/import/SampleDatasetList.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/data-explorer/internal/ui/import/SqlConnectionModal.tsx](../../react/src/modules/data-explorer/internal/ui/import/SqlConnectionModal.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/data-explorer/internal/ui/import/SqlRemoteTableSelectModal.tsx](../../react/src/modules/data-explorer/internal/ui/import/SqlRemoteTableSelectModal.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/data-explorer/internal/ui/import/SqliteTableSelectModal.tsx](../../react/src/modules/data-explorer/internal/ui/import/SqliteTableSelectModal.tsx) | 待查 | 待逐项阅读源码 | 待审查 |

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
| [modules/settings/internal/ui/KnowledgeSettings.tsx](../../react/src/modules/settings/internal/ui/KnowledgeSettings.tsx) | 迁移：保留显式项目文档索引管理 | 复用 Application `ProjectKnowledgeService`；原生设置尚缺此页 | 实现中；人工验收待完成 |
| [modules/settings/internal/ui/LanguageModelEditor.tsx](../../react/src/modules/settings/internal/ui/LanguageModelEditor.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/settings/internal/ui/LanguageModelProviderEditor.tsx](../../react/src/modules/settings/internal/ui/LanguageModelProviderEditor.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/settings/internal/ui/LanguageModelProviderSelect.tsx](../../react/src/modules/settings/internal/ui/LanguageModelProviderSelect.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/settings/internal/ui/LanguageModelSettings.tsx](../../react/src/modules/settings/internal/ui/LanguageModelSettings.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/settings/internal/ui/SettingsField.tsx](../../react/src/modules/settings/internal/ui/SettingsField.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/settings/internal/ui/SettingsPage.tsx](../../react/src/modules/settings/internal/ui/SettingsPage.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/settings/internal/ui/SettingsPageHeader.tsx](../../react/src/modules/settings/internal/ui/SettingsPageHeader.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/settings/internal/ui/SettingsView.tsx](../../react/src/modules/settings/internal/ui/SettingsView.tsx) | 迁移/复核：原生独立设置窗口替代 React Dialog | 原生模型页已有；知识库、外观及搜索/重置尚待逐项对照 | 审查中 |

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
