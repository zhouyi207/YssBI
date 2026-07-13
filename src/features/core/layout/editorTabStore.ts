import { create } from 'zustand';
import { immer } from 'zustand/middleware/immer';
import type { LayoutNode, LayoutTab, LayoutTree } from '@/shared/types/ui';
import {
  applyTabPinState,
  normalizeLayoutTab,
  normalizeLayoutTabs,
} from './layoutTabModel';
import { isEditorGroupNode } from './layoutEditorGroupNode';
import { listEditorGroupIds } from './editorGridLayout';
import { DEFAULT_EDITOR_GROUP_ID } from './workbenchLayoutDefaults';

/** One-shot import shape from pre-refactor layout nodes (not part of the live node model). */
interface LegacyEmbeddedNodeData {
  tabs?: LayoutTab[];
  activeTabId?: string | null;
  params?: { selectedNodeIds?: string[] };
}

function readLegacyNodeData(node: LayoutNode | undefined): LegacyEmbeddedNodeData | undefined {
  return node?.data as LegacyEmbeddedNodeData | undefined;
}

/** Per-group tab ordering and volatile editor chrome state. */
export interface EditorGroupPlacement {
  tabIds: string[];
  activeTabId: string | null;
  selectedNodeIds: string[];
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
  setSelectedNodeIds: (groupId: string, selectedNodeIds: string[]) => void;
  closeAllGraphTabs: () => void;
  mergePlacementsIntoGroup: (targetGroupId: string, sourceGroupIds: string[]) => void;
  renameTabId: (from: string, to: string) => void;

  snapshotMemento: () => EditorTabMemento;
  applyMemento: (memento: EditorTabMemento) => void;
  importFromLayoutNodes: (nodes: LayoutTree) => boolean;
  stripEmbeddedTabsFromNodes: (nodes: LayoutTree) => void;
}

const EMPTY_PLACEMENT: EditorGroupPlacement = {
  tabIds: [],
  activeTabId: null,
  selectedNodeIds: [],
};

function createEmptyPlacement(): EditorGroupPlacement {
  return { tabIds: [], activeTabId: null, selectedNodeIds: [] };
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
}

function readEmbeddedPlacement(node: LayoutNode): EditorGroupPlacement | null {
  const legacy = readLegacyNodeData(node);
  const tabs = legacy?.tabs;
  if (!tabs?.length && legacy?.activeTabId == null && !legacy?.params?.selectedNodeIds?.length) {
    return null;
  }
  const normalized = normalizeLayoutTabs(tabs ?? []);
  const lastTabId = normalized.length > 0 ? normalized[normalized.length - 1].id : null;
  return {
    tabIds: normalized.map((tab) => tab.id),
    activeTabId: legacy?.activeTabId ?? lastTabId,
    selectedNodeIds: legacy?.params?.selectedNodeIds ?? [],
  };
}

export function readLegacyEmbeddedTab(node: LayoutNode | undefined, tabId: string): LayoutTab | null {
  const legacyTab = readLegacyNodeData(node)?.tabs?.find((item) => item.id === tabId);
  return legacyTab ? normalizeLayoutTab(legacyTab) : null;
}

export function removeLegacyEmbeddedTab(node: LayoutNode | undefined, tabId: string): void {
  if (!node?.data) return;
  const legacy = readLegacyNodeData(node);
  if (!legacy?.tabs?.length) return;
  const nextTabs = legacy.tabs.filter((item) => item.id !== tabId);
  const legacyData = node.data as LegacyEmbeddedNodeData;
  if (nextTabs.length > 0) {
    legacyData.tabs = nextTabs;
    if (legacyData.activeTabId === tabId) {
      legacyData.activeTabId = nextTabs[nextTabs.length - 1]?.id ?? null;
    }
    return;
  }
  delete legacyData.tabs;
  delete legacyData.activeTabId;
}

export const useEditorTabStore = create<EditorTabState>()(
  immer((set, get) => ({
    registry: {},
    placements: {},

    getPlacement: (groupId) => get().placements[groupId] ?? EMPTY_PLACEMENT,

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
        if (placement.activeTabId !== tab.id) clearSelection(placement);
        placement.activeTabId = tab.id;
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
      clearSelection(placement);
      placement.activeTabId = tab.id;
    }),

    removeTab: (groupId, tabId) => set((state) => {
      const placement = state.placements[groupId];
      if (!placement) return;

      const closingIndex = placement.tabIds.indexOf(tabId);
      if (closingIndex === -1) return;

      placement.tabIds.splice(closingIndex, 1);

      if (placement.tabIds.length === 0) {
        placement.activeTabId = null;
        placement.selectedNodeIds = [];
        delete state.placements[groupId];
        pruneRegistry(state);
        return;
      }

      if (placement.activeTabId === tabId) {
        const nextIndex = Math.max(0, closingIndex - 1);
        placement.activeTabId = placement.tabIds[nextIndex] ?? null;
        clearSelection(placement);
      }
    }),

    moveTab: (sourceGroupId, tabId, targetGroupId, targetTabIndex) => set((state) => {
      const sourcePlacement = state.placements[sourceGroupId];
      if (!sourcePlacement?.tabIds.includes(tabId)) return;

      const tab = state.registry[tabId];
      if (!tab) return;

      registerTab(state, applyTabPinState(tab, true));

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
          sourcePlacement.activeTabId = sourcePlacement.tabIds[nextIndex] ?? null;
          if (sourcePlacement.activeTabId !== tabId) clearSelection(sourcePlacement);
        }
        if (sourcePlacement.tabIds.length === 0) {
          delete state.placements[sourceGroupId];
        }
        targetPlacement.activeTabId = tabId;
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
        sourcePlacement.activeTabId = tabId;
        return;
      }

      const closingIndex = sourcePlacement.tabIds.indexOf(tabId);
      sourcePlacement.tabIds.splice(closingIndex, 1);
      if (sourcePlacement.activeTabId === tabId) {
        const nextIndex = Math.max(0, closingIndex - 1);
        sourcePlacement.activeTabId = sourcePlacement.tabIds[nextIndex] ?? null;
        if (sourcePlacement.activeTabId !== tabId) clearSelection(sourcePlacement);
      }
      if (sourcePlacement.tabIds.length === 0) {
        delete state.placements[sourceGroupId];
      }

      if (targetTabIndex !== undefined) {
        targetPlacement.tabIds.splice(targetTabIndex, 0, tabId);
      } else {
        targetPlacement.tabIds.push(tabId);
      }
      targetPlacement.activeTabId = tabId;
    }),

    setActiveTab: (groupId, tabId) => set((state) => {
      const placement = state.placements[groupId];
      if (!placement || placement.tabIds.length === 0) return;

      const nextActiveId = tabId || null;
      if (placement.activeTabId === nextActiveId) return;
      if (nextActiveId != null && !placement.tabIds.includes(nextActiveId)) return;

      placement.activeTabId = nextActiveId;
      clearSelection(placement);
    }),

    setTabPinned: (groupId, tabId, pinned) => set((state) => {
      const placement = state.placements[groupId];
      if (!placement?.tabIds.includes(tabId)) return;
      const tab = state.registry[tabId];
      if (!tab) return;
      state.registry[tabId] = applyTabPinState(tab, pinned);
    }),

    setSelectedNodeIds: (groupId, selectedNodeIds) => set((state) => {
      if (!state.placements[groupId]) {
        state.placements[groupId] = createEmptyPlacement();
      }
      state.placements[groupId].selectedNodeIds = selectedNodeIds;
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
        placement.activeTabId = activeStillPresent
          ? placement.activeTabId
          : (remainingIds.length > 0 ? remainingIds[remainingIds.length - 1] : null);

        if (remainingIds.length === 0) {
          delete state.placements[groupId];
        }
      }
      pruneRegistry(state);
    }),

    mergePlacementsIntoGroup: (targetGroupId, sourceGroupIds) => set((state) => {
      if (!state.placements[targetGroupId]) {
        state.placements[targetGroupId] = createEmptyPlacement();
      }
      const target = state.placements[targetGroupId];
      const seen = new Set(target.tabIds);

      for (const sourceGroupId of sourceGroupIds) {
        if (sourceGroupId === targetGroupId) continue;
        const source = state.placements[sourceGroupId];
        if (!source) continue;
        for (const tabId of source.tabIds) {
          if (seen.has(tabId)) continue;
          seen.add(tabId);
          target.tabIds.push(tabId);
        }
        delete state.placements[sourceGroupId];
      }

      if (!target.activeTabId && target.tabIds.length > 0) {
        target.activeTabId = target.tabIds[target.tabIds.length - 1] ?? null;
      }
    }),

    renameTabId: (from, to) => set((state) => {
      const tab = state.registry[from];
      if (!tab || from === to) return;
      state.registry[to] = { ...tab, id: to };
      delete state.registry[from];
      for (const placement of Object.values(state.placements)) {
        placement.tabIds = placement.tabIds.map((tabId) => (tabId === from ? to : tabId));
        if (placement.activeTabId === from) placement.activeTabId = to;
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
        Object.entries(memento.placements).map(([groupId, placement]) => [
          groupId,
          {
            tabIds: [...placement.tabIds],
            activeTabId: placement.activeTabId,
            selectedNodeIds: [...placement.selectedNodeIds],
          },
        ]),
      );
    }),

    importFromLayoutNodes: (nodes) => {
      let changed = false;
      set((state) => {
        for (const node of Object.values(nodes)) {
          if (!isEditorGroupNode(node)) continue;
          const embedded = readEmbeddedPlacement(node);
          if (!embedded) {
            if (!state.placements[node.id]) {
              state.placements[node.id] = createEmptyPlacement();
            }
            continue;
          }
          changed = true;
          for (const tabId of embedded.tabIds) {
            const tab = readLegacyNodeData(node)?.tabs?.find((item) => item.id === tabId);
            if (tab) registerTab(state, tab);
          }
          state.placements[node.id] = {
            tabIds: [...embedded.tabIds],
            activeTabId: embedded.activeTabId,
            selectedNodeIds: [...embedded.selectedNodeIds],
          };
        }
        if (!state.placements[DEFAULT_EDITOR_GROUP_ID] && isEditorGroupNode(nodes[DEFAULT_EDITOR_GROUP_ID])) {
          state.placements[DEFAULT_EDITOR_GROUP_ID] = createEmptyPlacement();
        }
      });
      return changed;
    },

    stripEmbeddedTabsFromNodes: (nodes) => {
      for (const node of Object.values(nodes)) {
        if (!isEditorGroupNode(node) || !node.data) continue;
        const legacy = node.data as LegacyEmbeddedNodeData;
        delete legacy.tabs;
        delete legacy.activeTabId;
        if (legacy.params) delete legacy.params.selectedNodeIds;
      }
    },
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
