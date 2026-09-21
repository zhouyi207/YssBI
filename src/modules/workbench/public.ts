export type { EditorPanelScope, EditorRendererRegistry } from "./internal/layout/editorRenderer";
export { EditorResourcePanel } from "./internal/layout/EditorResourcePanel";
export * from "./internal/layout/index";
export { DEFAULT_LOGS_LAYOUT, LOGS_LAYOUT_COMPONENT_ID } from "./internal/layout/logsLayoutModel";
export type { WorkbenchPanelCommitToken } from "./internal/layout/workbenchTypes";
export { canRemoveWorkbenchPanel } from "./internal/layout/workbenchActivityGroup";
export type {
  RootPanelComponent,
  RootPanelProps,
  RootPanelActivationTarget,
  RootPanelRegistry,
  RootPanelTabComponent,
} from "./internal/layout/panelContribution";
export type {
  RootLayoutDndCoordinator,
  RootPanelActivationCoordinator,
} from "./internal/layout/RootLayoutHost";
export {
  RootPanelTabRenderer,
  type RootPanelTabActions,
  type RootPanelTabRendererProps,
  type WorkbenchTabTarget,
} from "./internal/layout/RootPanelTabRenderer";
export { WorkbenchWindow } from "./internal/ui/WorkbenchWindowEntry";
export type { WorkbenchOverlayRegistry } from "./internal/ui/overlay/overlayContribution";
export { useWorkbenchUiStore } from "./internal/state/workbenchUiStore";
export {
  clearEditorGroupGraphSelection,
  getEditorGroupGraphSelection,
  updateEditorGroupSelectedConnectionIds,
  updateEditorGroupSelectedNodeIds,
  type GraphSelection,
} from "./internal/state/editorPaneSelection";
export {
  commitEditorPanelPublication,
  commitWorkbenchPanelRemoval,
  releaseEditorPaneState,
  removeProjectScopedPanelsFromWorkbench,
  resetEditorPaneState,
} from "./internal/application/panelCommands";
export {
  resetWorkbenchLayout,
  openPluginWorkbenchView,
  syncPluginWorkbenchViews,
  revealWorkbenchView,
  toggleActivityWorkbenchGroup,
  toggleBottomWorkbenchGroup,
  toggleWorkbenchView,
} from "./internal/application/workbenchLayoutActions";
export {
  workbenchLayoutController,
  type ProjectResourcesReadyContext,
  type WorkbenchLayoutController,
} from "./internal/application/workbenchLayoutController";
export { showWorkbenchLayoutError } from "./internal/application/workbenchLayoutErrorFeedback";
export { ActivityPanelDocumentView } from "./internal/ui/activity/ActivityPanelDocumentView";
export { SidebarDragOverlay } from "./internal/ui/dnd/SidebarDragOverlay";
export { AboutModal } from "./internal/ui/menu/AboutModal";
export { ArchitectureModal } from "./internal/ui/menu/ArchitectureModal";
export { architectureSearch } from "./internal/ui/menu/architectureNavigation";
export {
  WorkbenchMenuBar,
  WorkbenchSemanticMenu,
  type WorkbenchMenuDefinition,
  type WorkbenchMenuItem,
  type WorkbenchThemeToggle,
  type WorkbenchWindowControls,
} from "./internal/ui/menu/WorkbenchMenuBar";
export { StatusBar } from "./internal/ui/status/StatusBar";
export { SidebarEmptyState } from "./internal/ui/sidebar/SidebarEmptyState";
export { SidebarRenameDialog } from "./internal/ui/sidebar/SidebarRenameDialog";
export type { SidebarInputDialogState } from "./internal/ui/sidebar/sidebarInputDialog";
export { useSidebarContextMenu } from "./internal/ui/sidebar/useSidebarContextMenu";
export {
  SidebarListItem,
  SidebarRowActionButton,
  SidebarTreeCategoryRow,
  SidebarTreeSearchInput,
  sidebarItemRowClass,
  SIDEBAR_ROW_ICON_SIZE,
  SIDEBAR_ROW_LEADING_SLOT_CLASS,
} from "./internal/ui/sidebar/primitives";
