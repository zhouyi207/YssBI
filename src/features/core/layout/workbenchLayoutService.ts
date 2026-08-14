import {
  editorDockviewPort,
  invalidateDockviewLayoutHydration,
  persistDockviewLayoutDebounced,
  persistDockviewLayoutNow,
} from '@/features/core/dockview';
import {
  DEFAULT_WORKBENCH_PANEL_SIZE,
  WORKBENCH_EDITOR_PART_ID,
  WORKBENCH_PANEL_PART_ID,
  useWorkbenchStore,
  workbenchGridPort,
} from '@/features/core/workbench';
import type { SidebarTabId } from '@/features/core/workbench';
import type { PanelViewId } from './panelPartModel';
import type { PanelPosition } from './panelPartLayout';
import type { WorkbenchPartId } from './workbenchLayoutDefaults';

let restoredPanelSize = DEFAULT_WORKBENCH_PANEL_SIZE;
let panelMaximized = false;

export function persistWorkbenchLayoutNow(): void { void persistDockviewLayoutNow(); }

export function collapseEditorGroupsForProjectSwitch(): void {
  void editorDockviewPort.reset();
  persistDockviewLayoutDebounced();
}


export function applyPanelPosition(position: PanelPosition): void {
  workbenchGridPort.movePart('panel', position === 'bottom' ? 'below' : position, 'editor');
  persistDockviewLayoutDebounced();
}

export function applyPanelPositionFromSetting(setting: string | undefined): void {
  const normalized = setting?.toLowerCase();
  applyPanelPosition(normalized === 'left' || normalized === 'right' ? normalized : 'bottom');
}

export function getPartSize(partId: WorkbenchPartId): number | undefined {
  return workbenchGridPort.getPartSize(partId);
}

export function resizePart(partId: WorkbenchPartId, size: number): void {
  workbenchGridPort.setPartSize(partId, size);
  persistDockviewLayoutDebounced();
}



export function setPanelActiveView(viewId: PanelViewId): void {
  useWorkbenchStore.getState().setPanelActiveView(viewId);
}

export function setWorkbenchPartVisible(
  partId: WorkbenchPartId,
  visible: boolean,
  options?: { userHidden?: boolean; persist?: boolean },
): void {
  if (useWorkbenchStore.getState().zenMode) return;
  const hidden = options?.userHidden ?? !visible;
  if (partId === 'sidebar') useWorkbenchStore.getState().setSidebarUserHidden(hidden);
  if (partId === 'panel') useWorkbenchStore.getState().setPanelUserHidden(hidden);
  if (partId === 'detail') useWorkbenchStore.getState().setDetailUserHidden(hidden);
  workbenchGridPort.setPartVisible(partId, visible);
  if (options?.persist !== false) persistDockviewLayoutDebounced();
}

export function togglePart(partId: WorkbenchPartId): void {
  setWorkbenchPartVisible(partId, !workbenchGridPort.getPartVisible(partId));
}

export function toggleSidebarVisibility(): void { useWorkbenchStore.getState().toggleSidebarVisibilityPreference(); }
export function toggleDetailVisibility(): void { useWorkbenchStore.getState().toggleDetailVisibilityPreference(); }
export function togglePanelVisibility(): void { useWorkbenchStore.getState().togglePanelVisibilityPreference(); }

export function togglePanelMaximized(): void {
  if (panelMaximized) {
    workbenchGridPort.setPartSize('panel', restoredPanelSize);
  } else {
    restoredPanelSize = workbenchGridPort.getPartSize('panel') ?? 200;
    workbenchGridPort.setPartSize('panel', Math.max(200, window.innerHeight * 0.75));
  }
  panelMaximized = !panelMaximized;
  persistDockviewLayoutDebounced();
}

export function showSidebarTab(tab: SidebarTabId): void {
  useWorkbenchStore.getState().showSidebarTab(tab);
}

export function toggleSidebarTab(tab: SidebarTabId): void {
  useWorkbenchStore.getState().toggleSidebarTab(tab);
}

export async function resetWorkbenchLayout(panelPosition: PanelPosition = 'bottom'): Promise<void> {
  invalidateDockviewLayoutHydration();
  useWorkbenchStore.getState().resetWorkbenchUIState();
  await editorDockviewPort.reset();
  workbenchGridPort.resetToDefault();
  if (panelPosition !== 'bottom') {
    workbenchGridPort.movePart(
      WORKBENCH_PANEL_PART_ID,
      panelPosition,
      WORKBENCH_EDITOR_PART_ID,
      DEFAULT_WORKBENCH_PANEL_SIZE,
    );
  }
  restoredPanelSize = DEFAULT_WORKBENCH_PANEL_SIZE;
  panelMaximized = false;
  persistDockviewLayoutDebounced();
}
