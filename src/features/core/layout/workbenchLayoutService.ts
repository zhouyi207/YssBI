import {
  editorDockviewPort,
  panelDockviewPort,
  invalidateDockviewLayoutHydration,
  persistDockviewLayoutDebounced,
  persistDockviewLayoutNow,
} from '@/features/core/dockview';
import {
  WORKBENCH_PANEL_PART_ID,
  useWorkbenchStore,
  workbenchGridPort,
} from '@/features/core/workbench';
import type { SidebarTabId } from '@/features/core/workbench';
import type { PanelViewId } from './panelPartModel';
import type { PanelPosition } from './panelPartLayout';
import type { WorkbenchPartId } from './workbenchLayoutDefaults';


export function setPanelCollapsed(
  collapsed: boolean,
  options?: { persist?: boolean },
): void {
  const state = useWorkbenchStore.getState();
  if (state.zenMode) return;

  state.setPanelCollapsed(collapsed);
  void panelDockviewPort.setCollapsed(collapsed);
  if (options?.persist !== false) persistDockviewLayoutDebounced();
}

export function togglePanelCollapsed(): void {
  setPanelCollapsed(!useWorkbenchStore.getState().panelCollapsed);
}

export function showPanelView(viewId: PanelViewId): void {
  void panelDockviewPort.activate(viewId);
  if (useWorkbenchStore.getState().panelCollapsed) setPanelCollapsed(false);
}

export function persistWorkbenchLayoutNow(): void { void persistDockviewLayoutNow(); }

export function collapseEditorGroupsForProjectSwitch(): void {
  void editorDockviewPort.reset();
  persistDockviewLayoutDebounced();
}

export function applyPanelPosition(position: PanelPosition): void {
  void panelDockviewPort.setPosition(position);
  persistDockviewLayoutDebounced();
}

export function applyPanelPositionFromSetting(setting: string | undefined): void {
  const normalized = setting?.toLowerCase();
  applyPanelPosition(normalized === 'left' || normalized === 'right' ? normalized : 'bottom');
}


export function setWorkbenchPartVisible(
  partId: WorkbenchPartId,
  visible: boolean,
  options?: { userHidden?: boolean; persist?: boolean },
): void {
  if (useWorkbenchStore.getState().zenMode) return;
  const hidden = options?.userHidden ?? !visible;
  if (partId === WORKBENCH_PANEL_PART_ID) {
    setPanelCollapsed(hidden, { persist: options?.persist });
    return;
  }
  if (partId === 'sidebar') useWorkbenchStore.getState().setSidebarUserHidden(hidden);
  if (partId === 'detail') useWorkbenchStore.getState().setDetailUserHidden(hidden);
  workbenchGridPort.setPartVisible(partId, visible);
  if (options?.persist !== false) persistDockviewLayoutDebounced();
}

export function togglePart(partId: WorkbenchPartId): void {
  if (partId === WORKBENCH_PANEL_PART_ID) {
    togglePanelCollapsed();
    return;
  }
  setWorkbenchPartVisible(partId, !workbenchGridPort.getPartVisible(partId));
}

export function toggleSidebarVisibility(): void { useWorkbenchStore.getState().toggleSidebarVisibilityPreference(); }
export function toggleDetailVisibility(): void { useWorkbenchStore.getState().toggleDetailVisibilityPreference(); }


export function showSidebarTab(tab: SidebarTabId): void {
  useWorkbenchStore.getState().showSidebarTab(tab);
}

export function toggleSidebarTab(tab: SidebarTabId): void {
  useWorkbenchStore.getState().toggleSidebarTab(tab);
}

export async function resetWorkbenchLayout(panelPosition: PanelPosition = 'bottom'): Promise<void> {
  invalidateDockviewLayoutHydration();
  useWorkbenchStore.getState().resetWorkbenchUIState();
  editorDockviewPort.unbind();
  panelDockviewPort.unbind();
  workbenchGridPort.resetToDefault();
  await panelDockviewPort.whenReady();
  await editorDockviewPort.whenReady();
  await editorDockviewPort.reset();
  await panelDockviewPort.setPosition(panelPosition);
  await panelDockviewPort.setCollapsed(false);
  persistDockviewLayoutDebounced();
}
