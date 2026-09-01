export type { EditorPanelScope, EditorRendererRegistry } from "./internal/dockview/editorRenderer";
export { EditorResourceDockPanel } from "./internal/dockview/EditorResourceDockPanel";
export type {
  RootDockviewPanelComponent,
  RootPanelRegistry,
} from "./internal/dockview/panelContribution";
export type { RootDockviewDndCoordinator } from "./internal/dockview/RootDockviewHost";
export { WorkbenchWindow } from "./internal/ui/WorkbenchWindowEntry";
export type { WorkbenchOverlayRegistry } from "./internal/ui/overlay/overlayContribution";
export { useWorkbenchUiStore } from "./internal/state/workbenchUiStore";
export {
  getWorkbenchUiSnapshot,
  subscribeWorkbenchUi,
  useWorkbenchUi,
  workbenchUi,
} from "./internal/state/ui";
export type { WorkbenchUiCapability } from "./internal/state/ui";
export { ActivityPanelShell } from "./internal/ui/activity/ActivityPanelShell";
export { SidebarEmptyState } from "./internal/ui/sidebar/SidebarEmptyState";
export { SidebarRenameDialog } from "./internal/ui/sidebar/SidebarRenameDialog";
export { SidebarSectionEmptyState } from "./internal/ui/sidebar/SidebarSectionEmptyState";
export { SidebarTabPanel } from "./internal/ui/sidebar/SidebarTabPanel";
export type { SidebarInputDialogState } from "./internal/ui/sidebar/sidebarInputDialog";
export { useSidebarContextMenu } from "./internal/ui/sidebar/useSidebarContextMenu";
export {
  SidebarChevron,
  SidebarDraggableItem,
  SidebarListItem,
  SidebarRowActionButton,
  SidebarTreeCategoryRow,
  SidebarTreeSearchInput,
  SidebarVirtualTree,
  sidebarGroupRowClass,
  sidebarItemIndent,
  sidebarItemLabelClass,
  sidebarItemRowClass,
  sidebarRowActionClass,
  sidebarVariableTypeBadgeClass,
  SIDEBAR_CHEVRON_SIZE,
  SIDEBAR_ROW_HEIGHT_CLASS,
  SIDEBAR_ROW_ICON_SIZE,
  SIDEBAR_ROW_LEADING_SLOT_CLASS,
  SIDEBAR_ROW_TRAILING_SLOT_CLASS,
  sidebarTreeSearchShellClass,
  type SidebarTreeCategoryRowProps,
  type SidebarTreeSearchInputProps,
  type SidebarVirtualTreeProps,
} from "./internal/ui/sidebar/primitives";
