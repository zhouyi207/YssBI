export { EditorGroupsService } from './editorGroupsService';
export {
  hydrateWorkbenchLayout,
  hydrateWorkbenchChrome,
  hydrateEditorGrid,
  getPartSize,
  resizePart,
  togglePart,
  persistWorkbenchLayoutDebounced,
  persistEditorGridDebounced,
  persistEditorGridNow,
  persistEditorTabsDebounced,
  persistEditorTabsNow,
  persistWorkbenchLayoutNow,
  collapseEditorGroupsForProjectSwitch,
  applyPanelPosition,
  applyPanelPositionFromSetting,
  resetWorkbenchLayout,
  setWorkbenchPartVisible,
  showSidebarTab,
  toggleDetailVisibility,
  togglePanelVisibility,
  toggleSidebarTab,
  toggleSidebarVisibility,
  setPanelActiveView,
} from './workbenchLayoutService';
export {
  enterZenMode,
  exitZenMode,
  isZenModeActive,
  toggleZenMode,
} from './workbenchZenMode';
export {
  clampWorkbenchPartSize,
  PANEL_MAX_VIEWPORT_RATIO,
  resolveWorkbenchPartMaxSize,
  resolveWorkbenchViewport,
} from './workbenchPanelSizing';
export {
  createInitialWorkbenchNodes,
  DEFAULT_EDITOR_GROUP_ID,
  WORKBENCH_PART_IDS,
} from './workbenchLayoutDefaults';
export type { WorkbenchPartId } from './workbenchLayoutDefaults';
export { useLayoutStore, SIDEBAR_NODE_ID, isSidebarTabId } from './layoutStore';
export type { LayoutState, SidebarTabId } from './layoutStore';
export { collectDirtyGraphTabs } from './tabDirty';
export {
  useEditorTabStore,
  getEditorGroupActiveTabId,
  getEditorGroupSelectedNodeIds,
  listEditorGroupTabIds,
  listAllOpenEditorTabs,
  isEditorGroupPlacementEmpty,
  reconcileEditorTabPlacements,
} from './editorTabStore';
export type { EditorGroupPlacement, EditorTabMemento } from './editorTabStore';
export { isEditorGroupNode } from './layoutEditorGroupNode';
export {
  getLayoutTabById,
  locateLayoutTab,
  getActiveLayoutTab,
  resolveEditorGroupId,
  resolveEditorTargetGroupId,
  updateEditorGroupSelectedNodeIds,
} from './layoutTabQueries';
export {
  isEditorGridSash,
  listEditorGroupIds,
  applyEqualGridSplit,
  splitEditorGroupInTree,
  removeEditorGroupFromTree,
} from './editorGridLayout';
export { panelFlexBasis } from './splitView';
export {
  applyEditorGridAddViewSizing,
  areEditorGridSplitChildrenDistributed,
  commitEditorGridLayoutState,
} from './editorGridSizing';
export {
  isWorkbenchPartUserHidden,
  shouldRestoreWorkbenchPartOnSashDrag,
} from './workbenchPartVisibility';
export type { PanelViewId } from './panelPartModel';
export { DEFAULT_PANEL_VIEWS } from './panelPartModel';
export {
  centerLayoutForPanelPosition,
  inferPanelPosition,
  isEditorPanelSash,
  normalizePanelPosition,
  panelPositionToSetting,
} from './panelPartLayout';
export type { PanelPosition, PanelPositionSetting } from './panelPartLayout';
export {
  buildGraphLayoutTab,
  buildWorksheetLayoutTab,
  findPreviewTabInTabs,
  isGraphLayoutTab,
  isPreviewLayoutTab,
  isWorksheetLayoutTab,
  layoutTabResourceRef,
  normalizeLayoutTab,
  normalizeLayoutTabs,
  readEditorGroupSnapshot,
  splitComponentForTab,
} from './layoutTabModel';
export type { LayoutTabInput } from './layoutTabModel';
export type { LocatedLayoutTab, LayoutGroupContext } from './layoutTabQueries';
export {
  readEditorPartOptions,
  preferSplitVerticallyFromDirection,
  DEFAULT_EDITOR_PART_OPTIONS,
} from './editorPartOptions';
export type { EditorPartOptions, OpenSideBySideDirection, EditorSplitSizingMode } from './editorPartOptions';
export {
  resolveEditorSplitHit,
  resolveEditorSplitHitFromClientPoint,
} from './editorSplitHitTest';
export type { EditorSplitHit, EditorSplitHitTestOptions } from './editorSplitHitTest';
export {
  isEditorDragCopyOperation,
  isEditorDragToggleSplitOperation,
  resolveEnableSplittingOnDrag,
} from './editorDragModifiers';
export {
  readEditorGroupDropBounds,
  findTabBarTargetFromPointer,
  findTabUnderPointer,
  findEditorGroupAtPointer,
  findCanvasDropGroupId,
} from './editorDropTarget';
export type { TabBarInsertPreviewContext } from './editorDropTarget';
export {
  findWorkbenchChromePartAtPointer,
  isPointerOverWorkbenchEditorSurface,
  isSidebarItemDropAllowedAtPointer,
  resolveWorkbenchDropSurfaceFlags,
  WORKBENCH_CHROME_PART_ATTR,
  WORKBENCH_EDITOR_SURFACE_ATTR,
  WORKBENCH_CHROME_PART_IDS,
} from './workbenchSidebarDropSurface';
export type { WorkbenchChromePartId } from './workbenchSidebarDropSurface';
export { getNextActiveEditorGroupId, prepareActiveGroupBeforeLastTabClose } from './editorGroupFocus';
