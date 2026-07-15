import { useLayoutStore, SIDEBAR_NODE_ID, isSidebarTabId, type SidebarTabId } from './layoutStore';
import {
  loadWorkbenchLayoutMemento,
  type WorkbenchLayoutMemento,
  type WorkbenchPartMemento,
} from './workbenchLayoutMemento';
import {
  WORKBENCH_PART_IDS,
  PANEL_PART_ID,
  type WorkbenchPartId,
} from './workbenchLayoutDefaults';
import { applyEditorGridMementoWithRepair, snapshotEditorGridMemento } from './editorGridMemento';
import { commitEditorGridLayoutState } from './editorGridSizing';
import { reconcileEditorTabPlacements, useEditorTabStore } from './editorTabStore';
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
import {
  flushWorkbenchLayoutPersist,
  mergeWorkbenchLayoutMemento,
  saveFullWorkbenchLayoutMemento,
  scheduleWorkbenchLayoutPersist,
  WORKBENCH_LAYOUT_PERSIST_DEBOUNCE_MS,
} from './workbenchLayoutPersistence';
import { clearZenModeSession, isZenModeActive } from './workbenchZenMode';

const PERSIST_DEBOUNCE_MS = WORKBENCH_LAYOUT_PERSIST_DEBOUNCE_MS;

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
  if ((partId === SIDEBAR_NODE_ID || partId === PANEL_PART_ID) && node.data?.userHidden === true) {
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

function snapshotWorkbenchChromeParts(): WorkbenchLayoutMemento['parts'] {
  const parts: WorkbenchLayoutMemento['parts'] = {};
  for (const partId of WORKBENCH_PART_IDS) {
    parts[partId] = readPartMemento(partId);
  }
  return parts;
}

export function snapshotWorkbenchLayoutMemento(): WorkbenchLayoutMemento {
  const state = useLayoutStore.getState();
  return {
    parts: snapshotWorkbenchChromeParts(),
    editorGrid: snapshotEditorGridMemento(state.nodes, state.activeEditorGroupId),
    editorTabs: useEditorTabStore.getState().snapshotMemento(),
  };
}

export function persistWorkbenchLayoutDebounced(delayMs = PERSIST_DEBOUNCE_MS): void {
  if (isZenModeActive()) return;

  const parts = snapshotWorkbenchChromeParts();
  scheduleWorkbenchLayoutPersist(
    'parts',
    () => mergeWorkbenchLayoutMemento({ parts }),
    delayMs,
  );
}

/** Persist only the editor grid slice (VS Code `workbench.editor.layout` decoupling). */
export function persistEditorGridDebounced(): void {
  scheduleWorkbenchLayoutPersist('editorGrid', () => {
    const state = useLayoutStore.getState();
    mergeWorkbenchLayoutMemento({
      editorGrid: snapshotEditorGridMemento(state.nodes, state.activeEditorGroupId),
    });
  });
}

export function persistEditorGridNow(): void {
  flushWorkbenchLayoutPersist('editorGrid', () => {
    const state = useLayoutStore.getState();
    mergeWorkbenchLayoutMemento({
      editorGrid: snapshotEditorGridMemento(state.nodes, state.activeEditorGroupId),
    });
  });
}

/** Persist runtime tab order/activation independently from the layout topology. */
export function persistEditorTabsDebounced(): void {
  scheduleWorkbenchLayoutPersist('editorTabs', () => {
    mergeWorkbenchLayoutMemento({
      editorTabs: useEditorTabStore.getState().snapshotMemento(),
    });
  });
}

export function persistEditorTabsNow(): void {
  flushWorkbenchLayoutPersist('editorTabs', () => {
    mergeWorkbenchLayoutMemento({
      editorTabs: useEditorTabStore.getState().snapshotMemento(),
    });
  });
}

export function persistWorkbenchLayoutNow(): void {
  saveFullWorkbenchLayoutMemento(snapshotWorkbenchLayoutMemento());
}

/** Collapse split groups on project switch and persist collapsed grid memento. */
export function collapseEditorGroupsForProjectSwitch(): void {
  useLayoutStore.getState().collapseEditorGroups();
  persistEditorGridNow();
}

export function hydrateWorkbenchChrome(): void {
  const memento = loadWorkbenchLayoutMemento();
  if (!memento?.parts) return;

  const { nodes, updateNode, resizeNode } = useLayoutStore.getState();
  const panelPosition = inferPanelPosition(nodes);

  for (const partId of WORKBENCH_PART_IDS) {
    const saved = memento.parts[partId];
    if (!saved) continue;

    const node = nodes[partId];
    if (!node) continue;

    if (saved.pixelSize != null) {
      resizeNode(partId, saved.pixelSize, panelPosition);
    }

    const nextData = { ...node.data };
    if (saved.visible != null) nextData.visible = saved.visible;
    if (partId === SIDEBAR_NODE_ID && saved.currentTab) nextData.currentTab = saved.currentTab;
    if (partId === 'detail' && saved.userHidden != null) nextData.userHidden = saved.userHidden;
    if ((partId === SIDEBAR_NODE_ID || partId === PANEL_PART_ID) && saved.userHidden != null) {
      nextData.userHidden = saved.userHidden;
    }
    if (partId === PANEL_PART_ID) {
      if (saved.maximized != null) nextData.maximized = saved.maximized;
      if (saved.restoredPixelSize != null) nextData.restoredPixelSize = saved.restoredPixelSize;
      if (saved.activePanelView) {
        nextData.activePanelView = saved.activePanelView === 'output' ? 'logs' : saved.activePanelView;
      }
    }

    updateNode(partId, { data: nextData });
  }
}

export function hydrateEditorGrid(): void {
  const memento = loadWorkbenchLayoutMemento();
  if (!memento?.editorGrid?.nodes?.length) return;

  const { setActiveGroup } = useLayoutStore.getState();
  useLayoutStore.setState((state) => {
    state.nodes = applyEditorGridMementoWithRepair(state.nodes, memento.editorGrid!);
    state.activeEditorGroupId = memento.editorGrid!.activeEditorGroupId;
  });
  useEditorTabStore.getState().applyMemento(
    memento.editorTabs ?? { registry: {}, placements: {} },
  );
  reconcileEditorTabPlacements(useLayoutStore.getState().nodes);
  setActiveGroup(memento.editorGrid!.activeEditorGroupId);
}

export function hydrateWorkbenchLayout(): void {
  hydrateWorkbenchChrome();
  hydrateEditorGrid();
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
    resizeNode(PANEL_PART_ID, clamped, position);
  }
  schedulePartResizeCommit(PANEL_PART_ID, clamped);
  useLayoutStore.setState((state) => {
    commitEditorGridLayoutState(state.nodes);
  });
  persistWorkbenchLayoutDebounced();
}

export function applyPanelPositionFromSetting(setting: string | undefined): void {
  applyPanelPosition(normalizePanelPosition(setting));
}

/** IWorkbenchLayoutService — chrome Part API (views should use this, not layoutStore.updateNode). */
export function getPartSize(partId: WorkbenchPartId): number | undefined {
  return useLayoutStore.getState().nodes[partId]?.pixelSize;
}

export function resizePart(partId: WorkbenchPartId, size: number): void {
  const state = useLayoutStore.getState();
  const panelPosition = inferPanelPosition(state.nodes);
  state.resizeNode(partId, size, panelPosition);
  const committedSize = useLayoutStore.getState().nodes[partId]?.pixelSize ?? size;
  schedulePartResizeCommit(partId, committedSize);
  persistWorkbenchLayoutDebounced();
}

export function reclampWorkbenchPanelSize(): void {
  const state = useLayoutStore.getState();
  const panel = state.nodes[PANEL_PART_ID];
  if (!panel?.pixelSize) return;

  const panelPosition = inferPanelPosition(state.nodes);
  const clamped = clampWorkbenchPartSize(
    panel,
    panel.pixelSize,
    resolveWorkbenchViewport(),
    panelPosition,
  );
  if (clamped === panel.pixelSize) return;

  state.resizeNode(PANEL_PART_ID, clamped, panelPosition);
  schedulePartResizeCommit(PANEL_PART_ID, clamped);
  if (!isZenModeActive()) {
    persistWorkbenchLayoutDebounced();
  }
}

/** Register the one window-level resize boundary; caller owns the returned lifecycle cleanup. */
export function subscribeWorkbenchViewportResize(delayMs = 100): () => void {
  let timer: ReturnType<typeof setTimeout> | null = null;
  const onResize = () => {
    if (timer) clearTimeout(timer);
    timer = setTimeout(() => {
      timer = null;
      reclampWorkbenchPanelSize();
      useLayoutStore.setState((state) => {
        commitEditorGridLayoutState(state.nodes);
      });
    }, delayMs);
  };

  window.addEventListener('resize', onResize);
  return () => {
    window.removeEventListener('resize', onResize);
    if (timer) clearTimeout(timer);
    timer = null;
  };
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
  options?: { userHidden?: boolean; persist?: boolean },
): void {
  if (isZenModeActive()) return;

  const node = useLayoutStore.getState().nodes[partId];
  if (!node) return;

  const nextData = { ...node.data, visible };
  if (options?.userHidden != null) {
    nextData.userHidden = options.userHidden;
  } else if (visible) {
    nextData.userHidden = false;
  }

  if (partId === PANEL_PART_ID && !visible && node.data?.maximized === true) {
    nextData.maximized = false;
    nextData.restoredPixelSize = undefined;
  }

  useLayoutStore.getState().updateNode(partId, { data: nextData });
  useLayoutStore.setState((state) => {
    commitEditorGridLayoutState(state.nodes);
  });
  if (options?.persist !== false) {
    persistWorkbenchLayoutDebounced();
  }
}

export function togglePart(partId: WorkbenchPartId): void {
  const node = useLayoutStore.getState().nodes[partId];
  if (!node) return;
  const isVisible = node.data?.visible !== false;
  setWorkbenchPartVisible(partId, !isVisible, { userHidden: isVisible });
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
  if (isZenModeActive()) return;

  const { nodes, updateNode, resizeNode } = useLayoutStore.getState();
  const panel = nodes[PANEL_PART_ID];
  if (!panel) return;

  const isMaximized = panel.data?.maximized === true;
  if (isMaximized) {
    const restored = panel.data?.restoredPixelSize ?? panel.pixelSize ?? 200;
    const panelPosition = inferPanelPosition(nodes);
    resizeNode(PANEL_PART_ID, restored, panelPosition);
    updateNode(PANEL_PART_ID, {
      data: { ...panel.data, maximized: false, restoredPixelSize: undefined },
    });
    const committedSize = useLayoutStore.getState().nodes[PANEL_PART_ID]?.pixelSize ?? restored;
    schedulePartResizeCommit(PANEL_PART_ID, committedSize);
    useLayoutStore.setState((state) => {
      commitEditorGridLayoutState(state.nodes);
    });
  } else {
    updateNode(PANEL_PART_ID, {
      data: {
        ...panel.data,
        maximized: true,
        restoredPixelSize: panel.pixelSize ?? 200,
      },
    });
    useLayoutStore.setState((state) => {
      commitEditorGridLayoutState(state.nodes);
    });
  }
  persistWorkbenchLayoutDebounced();
}

export function showSidebarTab(tab: SidebarTabId): void {
  if (isZenModeActive()) return;
  const wasHidden = useLayoutStore.getState().nodes[SIDEBAR_NODE_ID]?.data?.visible === false;
  useLayoutStore.getState().showSidebarTab(tab);
  if (wasHidden) {
    useLayoutStore.setState((state) => {
      commitEditorGridLayoutState(state.nodes);
    });
  }
  persistWorkbenchLayoutDebounced();
}

export function toggleSidebarTab(tab: SidebarTabId): void {
  if (isZenModeActive()) return;
  useLayoutStore.getState().toggleSidebarTab(tab);
  useLayoutStore.setState((state) => {
    commitEditorGridLayoutState(state.nodes);
  });
  persistWorkbenchLayoutDebounced();
}

export function resetWorkbenchLayout(panelPosition?: PanelPosition): void {
  clearZenModeSession();
  useLayoutStore.getState().resetWorkbenchLayout();
  if (panelPosition) {
    applyPanelPosition(panelPosition);
  }
  persistWorkbenchLayoutNow();
}
