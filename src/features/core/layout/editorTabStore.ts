import { create } from 'zustand';
import { immer } from 'zustand/middleware/immer';
import type { LayoutTab, LayoutTree } from '@/shared/types/ui';
import {
  applyTabPinState,
  normalizeLayoutTab,
  normalizeLayoutTabs,
} from './layoutTabModel';
import { listEditorGroupIds } from './editorGridLayout';
import {
  normalizePlacementGraphSelection,
  remapPlacementActiveTab,
  replacePlacementActiveTab,
} from './editorGraphSelectionPlacement';

/** Per-group tab ordering and volatile editor chrome state. */
export interface EditorGroupPlacement {
  tabIds: string[];
  activeTabId: string | null;
  selectedNodeIds: string[];
  selectedConnectionIds: string[];
  /** VS Code multi-tab selection within the group. */
  selectedTabIds: string[];
  /** VS Code locked editor group — tabs cannot leave the group. */
  locked?: boolean;
}

export interface EditorTabMemento {
  registry: Record<string, LayoutTab>;
  placements: Record<string, EditorGroupPlacement>;
}

export interface EditorTabState {
  registry: Record<string, LayoutTab>;
  placements: Record<string, EditorGroupPlacement>;

  getPlacement: (groupId: string) => EditorGroupPlacement;
  resolveTab: (tabId: string) => LayoutTab | null;
  resolveGroupTabs: (groupId: string) => LayoutTab[];
  locateTab: (tabId: string, groupId?: string) => { groupId: string; tab: LayoutTab } | null;
  isTabOpen: (tabId: string) => boolean;

  ensureGroupPlacement: (groupId: string) => void;
  initGroupPlacement: (
    groupId: string,
    tabs: LayoutTab[],
    activeTabId?: string | null,
  ) => void;
  removeGroupPlacement: (groupId: string) => void;

  addTab: (groupId: string, tab: LayoutTab, insertIndex?: number) => void;
  removeTab: (groupId: string, tabId: string) => void;
  moveTab: (
    sourceGroupId: string,
    tabId: string,
    targetGroupId: string,
    targetTabIndex?: number,
  ) => void;
  setActiveTab: (groupId: string, tabId: string | null) => void;
  setTabPinned: (groupId: string, tabId: string, pinned: boolean) => void;
  setTabSticky: (groupId: string, tabId: string, sticky: boolean) => void;
  setSelectedTabIds: (groupId: string, tabIds: string[]) => void;
  toggleTabInSelection: (groupId: string, tabId: string) => void;
  setGroupLocked: (groupId: string, locked: boolean) => void;
  setSelectedNodeIds: (groupId: string, selectedNodeIds: string[]) => void;
  setSelectedConnectionIds: (groupId: string, selectedConnectionIds: string[]) => void;
  clearGraphSelection: (groupId: string) => void;
  moveTabs: (
    sourceGroupId: string,
    tabIds: string[],
    targetGroupId: string,
    targetTabIndex?: number,
  ) => void;
  closeAllGraphTabs: () => void;
  mergePlacementsIntoGroup: (
    targetGroupId: string,
    sourceGroupIds: string[],
    insertIndex?: number,
  ) => void;
  /** VS Code copy editor — same tab id referenced in another group without removing source. */
  duplicateTabReference: (
    targetGroupId: string,
    tabId: string,
    insertIndex?: number,
  ) => void;
  duplicateGroupTabs: (
    sourceGroupId: string,
    targetGroupId: string,
    insertIndex?: number,
  ) => void;
  renameTabId: (from: string, to: string) => void;

  snapshotMemento: () => EditorTabMemento;
  applyMemento: (memento: EditorTabMemento) => void;
}

const EMPTY_PLACEMENT: EditorGroupPlacement = {
  tabIds: [],
  activeTabId: null,
  selectedNodeIds: [],
  selectedConnectionIds: [],
  selectedTabIds: [],
};

function createEmptyPlacement(): EditorGroupPlacement {
  return {
    tabIds: [],
    activeTabId: null,
    selectedNodeIds: [],
    selectedConnectionIds: [],
    selectedTabIds: [],
  };
}

function registerTab(state: { registry: Record<string, LayoutTab> }, tab: LayoutTab): void {
  state.registry[tab.id] = normalizeLayoutTab(tab);
}

function pruneRegistry(state: { registry: Record<string, LayoutTab>; placements: Record<string, EditorGroupPlacement> }): void {
  const referenced = new Set<string>();
  for (const placement of Object.values(state.placements)) {
    for (const tabId of placement.tabIds) referenced.add(tabId);
  }
  for (const tabId of Object.keys(state.registry)) {
    if (!referenced.has(tabId)) delete state.registry[tabId];
  }
}

function clearSelection(placement: EditorGroupPlacement): void {
  placement.selectedNodeIds = [];
  placement.selectedConnectionIds = [];
}

function uniqueIds(ids: readonly string[]): string[] {
  return [...new Set(ids)];
}

function setActiveGraph(placement: EditorGroupPlacement, activeTabId: string | null): void {
  replacePlacementActiveTab(placement, activeTabId);
}

export const useEditorTabStore = create<EditorTabState>()(
  immer((set, get) => ({
    registry: {},
    placements: {},

    getPlacement: (groupId) => {
      const placement = get().placements[groupId] ?? EMPTY_PLACEMENT;
      if (placement.selectedTabIds && placement.selectedConnectionIds) return placement;
      return {
        ...placement,
        selectedConnectionIds: placement.selectedConnectionIds ?? [],
        selectedTabIds: placement.selectedTabIds
          ?? (placement.activeTabId ? [placement.activeTabId] : []),
      };
    },

    resolveTab: (tabId) => get().registry[tabId] ?? null,

    resolveGroupTabs: (groupId) => {
      const placement = get().placements[groupId];
      if (!placement) return [];
      const { registry } = get();
      return placement.tabIds
        .map((tabId) => registry[tabId])
        .filter((tab): tab is LayoutTab => tab != null);
    },

    locateTab: (tabId, groupId) => {
      if (groupId) {
        const placement = get().placements[groupId];
        if (!placement?.tabIds.includes(tabId)) return null;
        const tab = get().registry[tabId];
        return tab ? { groupId, tab } : null;
      }
      for (const [gid, placement] of Object.entries(get().placements)) {
        if (!placement.tabIds.includes(tabId)) continue;
        const tab = get().registry[tabId];
        if (tab) return { groupId: gid, tab };
      }
      return null;
    },

    isTabOpen: (tabId) => {
      for (const placement of Object.values(get().placements)) {
        if (placement.tabIds.includes(tabId)) return true;
      }
      return false;
    },

    ensureGroupPlacement: (groupId) => set((state) => {
      if (!state.placements[groupId]) {
        state.placements[groupId] = createEmptyPlacement();
      }
    }),

    initGroupPlacement: (groupId, tabs, activeTabId) => set((state) => {
      const normalized = normalizeLayoutTabs(tabs);
      for (const tab of normalized) registerTab(state, tab);
      state.placements[groupId] = {
        tabIds: normalized.map((tab) => tab.id),
        activeTabId: activeTabId ?? (normalized.length > 0 ? normalized[normalized.length - 1].id : null),
        selectedNodeIds: [],
        selectedConnectionIds: [],
        selectedTabIds: activeTabId
          ? [activeTabId]
          : (normalized.length > 0 ? [normalized[normalized.length - 1].id] : []),
      };
    }),

    removeGroupPlacement: (groupId) => set((state) => {
      delete state.placements[groupId];
      pruneRegistry(state);
    }),

    addTab: (groupId, tab, insertIndex) => set((state) => {
      if (!state.placements[groupId]) {
        state.placements[groupId] = createEmptyPlacement();
      }
      const placement = state.placements[groupId];
      registerTab(state, tab);

      const existingIndex = placement.tabIds.indexOf(tab.id);
      if (existingIndex !== -1) {
        setActiveGraph(placement, tab.id);
        if (insertIndex !== undefined && insertIndex !== existingIndex) {
          const [existing] = placement.tabIds.splice(existingIndex, 1);
          const adjustedIndex = insertIndex > existingIndex ? insertIndex - 1 : insertIndex;
          placement.tabIds.splice(Math.max(0, Math.min(adjustedIndex, placement.tabIds.length)), 0, existing);
        }
        return;
      }

      if (insertIndex !== undefined && insertIndex >= 0 && insertIndex <= placement.tabIds.length) {
        placement.tabIds.splice(insertIndex, 0, tab.id);
      } else {
        placement.tabIds.push(tab.id);
      }
      setActiveGraph(placement, tab.id);
    }),

    removeTab: (groupId, tabId) => set((state) => {
      const placement = state.placements[groupId];
      if (!placement) return;

      const closingIndex = placement.tabIds.indexOf(tabId);
      if (closingIndex === -1) return;

      placement.tabIds.splice(closingIndex, 1);
      placement.selectedTabIds = (placement.selectedTabIds ?? []).filter((id) => id !== tabId);

      if (placement.tabIds.length === 0) {
        setActiveGraph(placement, null);
        delete state.placements[groupId];
        pruneRegistry(state);
        return;
      }

      if (placement.activeTabId === tabId) {
        const nextIndex = Math.max(0, closingIndex - 1);
        setActiveGraph(placement, placement.tabIds[nextIndex] ?? null);
      }
    }),

    moveTab: (sourceGroupId, tabId, targetGroupId, targetTabIndex) => set((state) => {
      const sourcePlacement = state.placements[sourceGroupId];
      if (!sourcePlacement?.tabIds.includes(tabId)) return;

      const tab = state.registry[tabId];
      if (!tab) return;

      if (sourceGroupId !== targetGroupId) {
        registerTab(state, applyTabPinState(tab, true));
      }

      if (!state.placements[targetGroupId]) {
        state.placements[targetGroupId] = createEmptyPlacement();
      }
      const targetPlacement = state.placements[targetGroupId];

      const existingIndex = targetPlacement.tabIds.indexOf(tabId);

      if (existingIndex !== -1 && sourceGroupId !== targetGroupId) {
        const sourceIndex = sourcePlacement.tabIds.indexOf(tabId);
        sourcePlacement.tabIds.splice(sourceIndex, 1);
        if (sourcePlacement.activeTabId === tabId) {
          const nextIndex = Math.max(0, sourceIndex - 1);
          setActiveGraph(sourcePlacement, sourcePlacement.tabIds[nextIndex] ?? null);
        }
        if (sourcePlacement.tabIds.length === 0) {
          delete state.placements[sourceGroupId];
        }
        setActiveGraph(targetPlacement, tabId);
        return;
      }

      if (sourceGroupId === targetGroupId) {
        const currentIndex = sourcePlacement.tabIds.indexOf(tabId);
        if (currentIndex === -1) return;
        const [removed] = sourcePlacement.tabIds.splice(currentIndex, 1);
        if (targetTabIndex !== undefined) {
          const adjustedIndex = targetTabIndex > currentIndex ? targetTabIndex - 1 : targetTabIndex;
          sourcePlacement.tabIds.splice(adjustedIndex, 0, removed);
        } else {
          sourcePlacement.tabIds.push(removed);
        }
        return;
      }

      const closingIndex = sourcePlacement.tabIds.indexOf(tabId);
      sourcePlacement.tabIds.splice(closingIndex, 1);
      if (sourcePlacement.activeTabId === tabId) {
        const nextIndex = Math.max(0, closingIndex - 1);
        setActiveGraph(sourcePlacement, sourcePlacement.tabIds[nextIndex] ?? null);
      }
      if (sourcePlacement.tabIds.length === 0) {
        delete state.placements[sourceGroupId];
      }

      if (targetTabIndex !== undefined) {
        targetPlacement.tabIds.splice(targetTabIndex, 0, tabId);
      } else {
        targetPlacement.tabIds.push(tabId);
      }
      setActiveGraph(targetPlacement, tabId);
    }),

    setActiveTab: (groupId, tabId) => set((state) => {
      const placement = state.placements[groupId];
      if (!placement || placement.tabIds.length === 0) return;

      const nextActiveId = tabId || null;
      if (placement.activeTabId === nextActiveId) return;
      if (nextActiveId != null && !placement.tabIds.includes(nextActiveId)) return;

      setActiveGraph(placement, nextActiveId);
      placement.selectedTabIds = nextActiveId ? [nextActiveId] : [];
    }),

    setTabPinned: (groupId, tabId, pinned) => set((state) => {
      const placement = state.placements[groupId];
      if (!placement?.tabIds.includes(tabId)) return;
      const tab = state.registry[tabId];
      if (!tab) return;
      state.registry[tabId] = applyTabPinState(tab, pinned);
    }),

    setTabSticky: (groupId, tabId, sticky) => set((state) => {
      const placement = state.placements[groupId];
      if (!placement?.tabIds.includes(tabId)) return;
      const tab = state.registry[tabId];
      if (!tab) return;
      state.registry[tabId] = { ...tab, sticky };
    }),

    setSelectedTabIds: (groupId, tabIds) => set((state) => {
      const placement = state.placements[groupId];
      if (!placement) return;
      placement.selectedTabIds = tabIds.filter((id) => placement.tabIds.includes(id));
    }),

    toggleTabInSelection: (groupId, tabId) => set((state) => {
      const placement = state.placements[groupId];
      if (!placement?.tabIds.includes(tabId)) return;
      const selected = new Set(placement.selectedTabIds ?? []);
      if (selected.has(tabId)) selected.delete(tabId);
      else selected.add(tabId);
      placement.selectedTabIds = placement.tabIds.filter((id) => selected.has(id));
      setActiveGraph(placement, tabId);
    }),

    setGroupLocked: (groupId, locked) => set((state) => {
      if (!state.placements[groupId]) {
        state.placements[groupId] = createEmptyPlacement();
      }
      state.placements[groupId].locked = locked;
    }),

    moveTabs: (sourceGroupId, tabIds, targetGroupId, targetTabIndex) => {
      const store = get();
      const sourceOrder = store.getPlacement(sourceGroupId).tabIds;
      const orderedIds = sourceOrder.filter((id) => tabIds.includes(id));
      orderedIds.forEach((tabId, offset) => {
        const insertAt = targetTabIndex !== undefined ? targetTabIndex + offset : undefined;
        store.moveTab(sourceGroupId, tabId, targetGroupId, insertAt);
      });
    },

    setSelectedNodeIds: (groupId, selectedNodeIds) => set((state) => {
      if (!state.placements[groupId]) {
        state.placements[groupId] = createEmptyPlacement();
      }
      const placement = state.placements[groupId];
      placement.selectedNodeIds = uniqueIds(selectedNodeIds);
      placement.selectedConnectionIds = [];
    }),

    setSelectedConnectionIds: (groupId, selectedConnectionIds) => set((state) => {
      if (!state.placements[groupId]) {
        state.placements[groupId] = createEmptyPlacement();
      }
      const placement = state.placements[groupId];
      placement.selectedNodeIds = [];
      placement.selectedConnectionIds = uniqueIds(selectedConnectionIds);
    }),

    clearGraphSelection: (groupId) => set((state) => {
      const placement = state.placements[groupId];
      if (placement) clearSelection(placement);
    }),

    closeAllGraphTabs: () => set((state) => {
      for (const [groupId, placement] of Object.entries(state.placements)) {
        const remainingIds = placement.tabIds.filter((tabId) => {
          const tab = state.registry[tabId];
          return tab
            && tab.type !== 'event'
            && tab.type !== 'function'
            && tab.type !== 'worksheet';
        });
        if (remainingIds.length === placement.tabIds.length) continue;

        placement.tabIds = remainingIds;
        const activeStillPresent = placement.activeTabId != null && remainingIds.includes(placement.activeTabId);
        setActiveGraph(
          placement,
          activeStillPresent
            ? placement.activeTabId
            : (remainingIds.length > 0 ? remainingIds[remainingIds.length - 1] : null),
        );

        if (remainingIds.length === 0) {
          delete state.placements[groupId];
        }
      }
      pruneRegistry(state);
    }),

    mergePlacementsIntoGroup: (targetGroupId, sourceGroupIds, insertIndex) => set((state) => {
      if (!state.placements[targetGroupId]) {
        state.placements[targetGroupId] = createEmptyPlacement();
      }
      const target = state.placements[targetGroupId];
      const seen = new Set(target.tabIds);
      const mergedIds: string[] = [];
      let activeFromSource: string | null = null;

      for (const sourceGroupId of sourceGroupIds) {
        if (sourceGroupId === targetGroupId) continue;
        const source = state.placements[sourceGroupId];
        if (!source) continue;
        if (
          !activeFromSource
          && source.activeTabId
          && source.tabIds.includes(source.activeTabId)
        ) {
          activeFromSource = source.activeTabId;
        }
        for (const tabId of source.tabIds) {
          if (seen.has(tabId)) continue;
          seen.add(tabId);
          mergedIds.push(tabId);
        }
        delete state.placements[sourceGroupId];
      }

      if (mergedIds.length === 0) return;

      if (activeFromSource && !mergedIds.includes(activeFromSource)) {
        activeFromSource = mergedIds[mergedIds.length - 1] ?? null;
      }

      if (insertIndex !== undefined) {
        target.tabIds.splice(insertIndex, 0, ...mergedIds);
      } else {
        target.tabIds.push(...mergedIds);
      }

      if (activeFromSource) {
        setActiveGraph(target, activeFromSource);
      } else if (!target.activeTabId && target.tabIds.length > 0) {
        setActiveGraph(target, target.tabIds[target.tabIds.length - 1] ?? null);
      }
    }),

    duplicateTabReference: (targetGroupId, tabId, insertIndex) => set((state) => {
      const tab = state.registry[tabId];
      if (!tab) return;
      registerTab(state, applyTabPinState(tab, true));
      if (!state.placements[targetGroupId]) {
        state.placements[targetGroupId] = createEmptyPlacement();
      }
      const target = state.placements[targetGroupId];
      if (!target.tabIds.includes(tabId)) {
        if (insertIndex !== undefined) {
          target.tabIds.splice(insertIndex, 0, tabId);
        } else {
          target.tabIds.push(tabId);
        }
      }
      setActiveGraph(target, tabId);
    }),

    duplicateGroupTabs: (sourceGroupId, targetGroupId, insertIndex) => set((state) => {
      const source = state.placements[sourceGroupId];
      if (!source) return;
      if (!state.placements[targetGroupId]) {
        state.placements[targetGroupId] = createEmptyPlacement();
      }
      const target = state.placements[targetGroupId];
      let cursor = insertIndex ?? target.tabIds.length;
      for (const tabId of source.tabIds) {
        const tab = state.registry[tabId];
        if (!tab) continue;
        registerTab(state, applyTabPinState(tab, true));
        if (target.tabIds.includes(tabId)) {
          setActiveGraph(target, tabId);
          continue;
        }
        target.tabIds.splice(cursor, 0, tabId);
        cursor += 1;
        setActiveGraph(target, tabId);
      }
    }),

    renameTabId: (from, to) => set((state) => {
      const tab = state.registry[from];
      if (!tab || from === to) return;
      state.registry[to] = { ...tab, id: to };
      delete state.registry[from];
      for (const placement of Object.values(state.placements)) {
        placement.tabIds = placement.tabIds.map((tabId) => (tabId === from ? to : tabId));
        remapPlacementActiveTab(placement, from, to);
      }
    }),

    snapshotMemento: () => {
      const { registry, placements } = get();
      return {
        registry: { ...registry },
        placements: Object.fromEntries(
          Object.entries(placements).map(([groupId, placement]) => [
            groupId,
            {
              tabIds: [...placement.tabIds],
              activeTabId: placement.activeTabId,
              selectedNodeIds: [...placement.selectedNodeIds],
              selectedConnectionIds: [...(placement.selectedConnectionIds ?? [])],
              selectedTabIds: [...(placement.selectedTabIds ?? [])],
              locked: placement.locked,
            },
          ]),
        ),
      };
    },

    applyMemento: (memento) => set((state) => {
      state.registry = Object.fromEntries(
        Object.entries(memento.registry).map(([tabId, tab]) => [tabId, normalizeLayoutTab(tab)]),
      );
      state.placements = Object.fromEntries(
        Object.entries(memento.placements).map(([groupId, placement]) => {
          const selection = normalizePlacementGraphSelection(placement);
          return [
            groupId,
            {
              tabIds: [...placement.tabIds],
              activeTabId: placement.activeTabId,
              ...selection,
              selectedTabIds: [...(placement.selectedTabIds ?? [])],
              locked: placement.locked,
            },
          ];
        }),
      );
    }),

  })),
);

/** Editor group has no open tabs in the placement store. */
export function isEditorGroupPlacementEmpty(groupId: string): boolean {
  return (useEditorTabStore.getState().placements[groupId]?.tabIds.length ?? 0) === 0;
}

/** Prune placements for groups that no longer exist in the layout tree. */
export function reconcileEditorTabPlacements(nodes: LayoutTree): void {
  const liveGroupIds = new Set(listEditorGroupIds(nodes));
  useEditorTabStore.setState((state) => {
    for (const groupId of Object.keys(state.placements)) {
      if (!liveGroupIds.has(groupId)) {
        delete state.placements[groupId];
      }
    }
    for (const groupId of liveGroupIds) {
      if (!state.placements[groupId]) {
        state.placements[groupId] = createEmptyPlacement();
      }
    }
    pruneRegistry(state);
  });
}

export function getEditorGroupActiveTabId(groupId: string): string | null {
  return useEditorTabStore.getState().getPlacement(groupId).activeTabId;
}

export function getEditorGroupSelectedNodeIds(groupId: string): string[] {
  return useEditorTabStore.getState().getPlacement(groupId).selectedNodeIds;
}

export function listEditorGroupTabIds(groupId: string): string[] {
  return useEditorTabStore.getState().getPlacement(groupId).tabIds;
}

export function listAllOpenEditorTabs(): Array<{ groupId: string; tab: LayoutTab }> {
  const state = useEditorTabStore.getState();
  const entries: Array<{ groupId: string; tab: LayoutTab }> = [];
  for (const [groupId, placement] of Object.entries(state.placements)) {
    for (const tabId of placement.tabIds) {
      const tab = state.registry[tabId];
      if (tab) entries.push({ groupId, tab });
    }
  }
  return entries;
}
