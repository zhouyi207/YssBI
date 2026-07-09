import { useLayoutStore, SIDEBAR_NODE_ID, isSidebarTabId, type SidebarTabId } from './layoutStore';
import {
  loadWorkbenchLayoutMemento,
  saveWorkbenchLayoutMemento,
  type WorkbenchLayoutMemento,
  type WorkbenchPartMemento,
} from './workbenchLayoutMemento';
import {
  WORKBENCH_PART_IDS,
  PANEL_PART_ID,
  type WorkbenchPartId,
} from './workbenchLayoutDefaults';
import { applyEditorGridMementoWithRepair, snapshotEditorGridMemento } from './editorGridMemento';
import { schedulePartResizeCommit } from './partResizeNotifier';
import type { PanelViewId } from './panelPartModel';
import {
  centerLayoutForPanelPosition,
  inferPanelPosition,
  normalizePanelPosition,
  type PanelPosition,
} from './panelPartLayout';
import {
  clampWorkbenchPartSize,
  resolveWorkbenchViewport,
} from './workbenchPanelSizing';
import { SASH_DRAG_END_EVENT } from '@/views/EditorView/Renderer/sashResizeLogic';

const PERSIST_DEBOUNCE_MS = 250;

let persistTimer: ReturnType<typeof setTimeout> | null = null;

function readPartMemento(partId: WorkbenchPartId): WorkbenchPartMemento {
  const node = useLayoutStore.getState().nodes[partId];
  if (!node) return {};

  const memento: WorkbenchPartMemento = {
    pixelSize: node.pixelSize,
    visible: node.data?.visible !== false,
  };

  if (partId === SIDEBAR_NODE_ID && isSidebarTabId(node.data?.currentTab)) {
    memento.currentTab = node.data!.currentTab!;
  }

  if (partId === 'detail' && node.data?.userHidden === true) {
    memento.userHidden = true;
  }

  if (partId === PANEL_PART_ID) {
    if (node.data?.maximized === true) memento.maximized = true;
    if (node.data?.restoredPixelSize != null) {
      memento.restoredPixelSize = node.data.restoredPixelSize;
    }
    if (node.data?.activePanelView) {
      memento.activePanelView = node.data.activePanelView as PanelViewId;
    }
  }

  return memento;
}

export function snapshotWorkbenchLayoutMemento(): WorkbenchLayoutMemento {
  const state = useLayoutStore.getState();
  const parts: WorkbenchLayoutMemento['parts'] = {};
  for (const partId of WORKBENCH_PART_IDS) {
    parts[partId] = readPartMemento(partId);
  }
  return {
    parts,
    editorGrid: snapshotEditorGridMemento(state.nodes, state.activeEditorGroupId),
  };
}

function schedulePersist(): void {
  if (persistTimer) clearTimeout(persistTimer);
  persistTimer = setTimeout(() => {
    persistTimer = null;
    saveWorkbenchLayoutMemento(snapshotWorkbenchLayoutMemento());
  }, PERSIST_DEBOUNCE_MS);
}

export function persistWorkbenchLayoutDebounced(delayMs = PERSIST_DEBOUNCE_MS): void {
  if (persistTimer) clearTimeout(persistTimer);
  persistTimer = setTimeout(() => {
    persistTimer = null;
    saveWorkbenchLayoutMemento(snapshotWorkbenchLayoutMemento());
  }, delayMs);
}

export function persistEditorGridDebounced(): void {
  schedulePersist();
}

export function persistWorkbenchLayoutNow(): void {
  if (persistTimer) {
    clearTimeout(persistTimer);
    persistTimer = null;
  }
  saveWorkbenchLayoutMemento(snapshotWorkbenchLayoutMemento());
}

export function applyPanelPosition(position: PanelPosition): void {
  const { nodes, updateNode, resizeNode } = useLayoutStore.getState();
  if (inferPanelPosition(nodes) === position) return;

  const center = nodes.center;
  const panel = nodes[PANEL_PART_ID];
  if (!center || !panel) return;

  const layout = centerLayoutForPanelPosition(position);
  const panelData = { ...panel.data };
  if (panelData.maximized === true) {
    panelData.maximized = false;
    delete panelData.restoredPixelSize;
  }

  updateNode(PANEL_PART_ID, { data: panelData });
  updateNode('center', {
    type: layout.type,
    children: layout.children,
  });

  const clamped = clampWorkbenchPartSize(
    panel,
    panel.pixelSize ?? 200,
    resolveWorkbenchViewport(),
    position,
  );
  if (clamped !== panel.pixelSize) {
    resizeNode(PANEL_PART_ID, clamped);
  }
  schedulePartResizeCommit(PANEL_PART_ID, clamped);
  persistWorkbenchLayoutDebounced();
}

export function applyPanelPositionFromSetting(setting: string | undefined): void {
  applyPanelPosition(normalizePanelPosition(setting));
}

export function hydrateWorkbenchLayout(): void {
  const memento = loadWorkbenchLayoutMemento();
  if (!memento?.parts) return;

  const { nodes, updateNode, resizeNode, setActiveGroup } = useLayoutStore.getState();

  for (const partId of WORKBENCH_PART_IDS) {
    const saved = memento.parts[partId];
    if (!saved) continue;

    const node = nodes[partId];
    if (!node) continue;

    if (saved.pixelSize != null) {
      resizeNode(partId, saved.pixelSize);
    }

    const nextData = { ...node.data };
    if (saved.visible != null) nextData.visible = saved.visible;
    if (partId === SIDEBAR_NODE_ID && saved.currentTab) nextData.currentTab = saved.currentTab;
    if (partId === 'detail' && saved.userHidden != null) nextData.userHidden = saved.userHidden;
    if (partId === PANEL_PART_ID) {
      if (saved.maximized != null) nextData.maximized = saved.maximized;
      if (saved.restoredPixelSize != null) nextData.restoredPixelSize = saved.restoredPixelSize;
      if (saved.activePanelView) nextData.activePanelView = saved.activePanelView;
    }

    updateNode(partId, { data: nextData });
  }

  if (memento.editorGrid?.nodes?.length) {
    useLayoutStore.setState((state) => {
      state.nodes = applyEditorGridMementoWithRepair(state.nodes, memento.editorGrid!);
      state.activeEditorGroupId = memento.editorGrid!.activeEditorGroupId;
    });
    setActiveGroup(memento.editorGrid.activeEditorGroupId);
  }
}

/** IWorkbenchLayoutService — chrome Part API (views should use this, not layoutStore.updateNode). */
export function getPartSize(partId: WorkbenchPartId): number | undefined {
  return useLayoutStore.getState().nodes[partId]?.pixelSize;
}

export function resizePart(partId: WorkbenchPartId, size: number): void {
  useLayoutStore.getState().resizeNode(partId, size);
  schedulePartResizeCommit(partId, size);
  persistWorkbenchLayoutDebounced();
}

export function setPanelActiveView(viewId: PanelViewId): void {
  const node = useLayoutStore.getState().nodes[PANEL_PART_ID];
  if (!node) return;
  useLayoutStore.getState().updateNode(PANEL_PART_ID, {
    data: { ...node.data, activePanelView: viewId },
  });
  persistWorkbenchLayoutDebounced();
}

export function setWorkbenchPartVisible(
  partId: WorkbenchPartId,
  visible: boolean,
  options?: { userHidden?: boolean },
): void {
  const node = useLayoutStore.getState().nodes[partId];
  if (!node) return;

  const nextData = { ...node.data, visible };
  if (partId === 'detail' && options?.userHidden != null) {
    nextData.userHidden = options.userHidden;
  }

  useLayoutStore.getState().updateNode(partId, { data: nextData });
  persistWorkbenchLayoutDebounced();
}

export function togglePart(partId: WorkbenchPartId): void {
  const node = useLayoutStore.getState().nodes[partId];
  if (!node) return;
  const isVisible = node.data?.visible !== false;
  if (partId === 'detail') {
    setWorkbenchPartVisible(partId, !isVisible, { userHidden: isVisible });
    return;
  }
  setWorkbenchPartVisible(partId, !isVisible);
}

export function toggleSidebarVisibility(): void {
  togglePart(SIDEBAR_NODE_ID);
}

export function toggleDetailVisibility(): void {
  togglePart('detail');
}

export function togglePanelVisibility(): void {
  togglePart(PANEL_PART_ID);
}

export function togglePanelMaximized(): void {
  const { nodes, updateNode } = useLayoutStore.getState();
  const panel = nodes[PANEL_PART_ID];
  if (!panel) return;

  const isMaximized = panel.data?.maximized === true;
  if (isMaximized) {
    const restored = panel.data?.restoredPixelSize ?? panel.pixelSize ?? 200;
    updateNode(PANEL_PART_ID, {
      pixelSize: restored,
      data: { ...panel.data, maximized: false, restoredPixelSize: undefined },
    });
  } else {
    updateNode(PANEL_PART_ID, {
      data: {
        ...panel.data,
        maximized: true,
        restoredPixelSize: panel.pixelSize ?? 200,
      },
    });
  }
  persistWorkbenchLayoutDebounced();
}

export function showSidebarTab(tab: SidebarTabId): void {
  useLayoutStore.getState().showSidebarTab(tab);
  persistWorkbenchLayoutDebounced();
}

export function toggleSidebarTab(tab: SidebarTabId): void {
  useLayoutStore.getState().toggleSidebarTab(tab);
  persistWorkbenchLayoutDebounced();
}

export function resetWorkbenchLayout(panelPosition?: PanelPosition): void {
  useLayoutStore.getState().resetWorkbenchLayout();
  if (panelPosition) {
    applyPanelPosition(panelPosition);
  }
  persistWorkbenchLayoutNow();
}

export function subscribeWorkbenchLayoutPersistence(): () => void {
  const onSashEnd = () => persistWorkbenchLayoutDebounced();
  window.addEventListener(SASH_DRAG_END_EVENT, onSashEnd);
  return () => window.removeEventListener(SASH_DRAG_END_EVENT, onSashEnd);
}
