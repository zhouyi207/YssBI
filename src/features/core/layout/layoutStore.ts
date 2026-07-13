import { create } from 'zustand';
import { immer } from 'zustand/middleware/immer';
import { LayoutNode, LayoutTree, LayoutTab } from '@/shared/types/ui';
import { getActiveLayoutTab } from './layoutTabQueries';
import type { EditorSplitEdge } from './editorSplitLayout';
import { clampWorkbenchPartSize } from './workbenchPanelSizing';
import { inferPanelPosition, type PanelPosition } from './panelPartLayout';
import {
    createInitialWorkbenchNodes,
    DEFAULT_EDITOR_GROUP_ID,
    EDITOR_AREA_ID,
    WORKBENCH_ROOT_ID,
} from './workbenchLayoutDefaults';
import {
    applyEditorGridPixelSizes,
    applyEqualGridSplit,
    clearEditorGroupMaximizedHidden,
    firstEditorGroupId,
    isActiveEditorGroupValid,
    listEditorGroupIds,
    readEditorAreaMaximizedGroupId,
    readEditorAreaRestoredGridSizes,
    removeEditorGroupFromTree,
    setEditorGroupMaximizedHidden,
    snapshotEditorGridPixelSizes,
    splitEditorGroupInTree,
    writeEditorAreaMaximizeState,
} from './editorGridLayout';
import { commitSplitPairSizes, normalizeEditorGridSplitWeights } from './editorGridSizing';
import {
    isEditorGroupPlacementEmpty,
    readLegacyEmbeddedTab,
    removeLegacyEmbeddedTab,
    reconcileEditorTabPlacements,
    useEditorTabStore,
} from './editorTabStore';

export type SidebarTabId = 'graphs' | 'nodes' | 'variables' | 'data' | 'commands' | 'charts';

export const SIDEBAR_NODE_ID = 'sidebar';

export function isSidebarTabId(value: string | null | undefined): value is SidebarTabId {
    return (
        value === 'graphs'
        || value === 'nodes'
        || value === 'variables'
        || value === 'data'
        || value === 'commands'
        || value === 'charts'
    );
}

export interface LayoutState {
    rootId: string;
    nodes: LayoutTree;

    // Actions
    getNode: (id: string) => LayoutNode | undefined;

    // Basic tree manipulation
    addNode: (node: LayoutNode) => void;
    updateNode: (id: string, patches: Partial<LayoutNode>) => void;

    // High level actions
    splitEditorGroupAtEdge: (
        targetGroupId: string,
        edge: EditorSplitEdge,
        payload: {
            component: string;
            tabs: LayoutTab[];
            activeTabId?: string;
            pinSourceActiveTab?: boolean;
        },
    ) => string | null;
    resizeNode: (nodeId: string, size: number, panelPosition?: PanelPosition) => void;

    /** Show sidebar and switch activity tab. pixelSize is preserved while hidden. */
    showSidebarTab: (tab: SidebarTabId) => void;
    /** Toggle sidebar visibility; same tab click hides, different tab switches. */
    toggleSidebarTab: (tab: SidebarTabId) => void;

    /** Collapse split editor groups back to a single default group. */
    collapseEditorGroups: () => void;
    /** Remove an empty editor group through the editor-grid domain boundary. */
    removeEditorGroup: (groupId: string) => boolean;
    /** Reset workbench chrome while preserving the complete editor grid session. */
    resetWorkbenchLayout: () => void;

    // Tab placement (delegates to editorTabStore)
    moveTab: (sourceNodeId: string, tabId: string, targetNodeId: string, targetTabIndex?: number) => void;
    removeTab: (nodeId: string, tabId: string) => void;
    addTab: (nodeId: string, tab: LayoutTab, insertIndex?: number) => void;
    setTabPinned: (nodeId: string, tabId: string, pinned: boolean) => void;
    setEditorGroupActiveTab: (nodeId: string, tabId: string | null) => void;
    /** Pixelize both flex siblings after first sash drag between editor groups. */
    commitFlexSplitResize: (beforeId: string, afterId: string, beforeSize: number, afterSize: number) => void;
    /** VS Code maximize editor group — hide sibling groups until toggled off. */
    toggleMaximizeEditorGroup: (groupId: string) => void;
    /** Reset a flex/pixel split between two editor-grid siblings to equal sizes. */
    resetEditorGridSplitEqual: (beforeId: string, afterId: string, beforeSize: number, afterSize: number) => void;
    closeAllGraphTabs: () => void;

    // UI State
    isDragging: boolean;
    setDragging: (isDragging: boolean) => void;
    activeEditorGroupId: string | null;
    isSettingsOpen: boolean;
    setActiveGroup: (id: string | null) => void;
    openSettings: () => void;
    closeSettings: () => void;
    setSettingsOpen: (open: boolean) => void;
    isAltPressed: boolean;
    setAltPressed: (pressed: boolean) => void;
    /** Session-only zen mode — hides shell chrome + workbench parts without persisting visibility. */
    zenMode: boolean;
}

function removeEmptyEditorGroupIfNeeded(
    state: { nodes: LayoutTree; activeEditorGroupId: string | null },
    groupId: string,
    preferActiveGroupId?: string,
): void {
    const { removed, nextActiveGroupId } = removeEditorGroupFromTree(state.nodes, groupId);
    if (!removed) return;
    useEditorTabStore.getState().removeGroupPlacement(groupId);
    if (state.activeEditorGroupId === groupId) {
        state.activeEditorGroupId = preferActiveGroupId ?? nextActiveGroupId;
    }
}

function ensureValidActiveEditorGroup(state: { nodes: LayoutTree; activeEditorGroupId: string | null }): void {
    if (isActiveEditorGroupValid(state.nodes, state.activeEditorGroupId)) return;
    state.activeEditorGroupId = firstEditorGroupId(state.nodes);
}

function collectDescendantIds(nodes: LayoutTree, rootId: string, skipId?: string): string[] {
    const collected: string[] = [];
    const visit = (id: string) => {
        if (id === skipId) return;
        const node = nodes[id];
        if (!node) return;
        collected.push(id);
        node.children?.forEach(visit);
    };
    const root = nodes[rootId];
    root?.children?.forEach(visit);
    return collected;
}

const INITIAL_NODES = createInitialWorkbenchNodes();

export const useLayoutStore = create<LayoutState>()(
    immer((set, get) => ({
        rootId: WORKBENCH_ROOT_ID,
        nodes: INITIAL_NODES,
        isDragging: false,
        activeEditorGroupId: DEFAULT_EDITOR_GROUP_ID,

        getNode: (id) => get().nodes[id],

        addNode: (node) => set((state) => {
            state.nodes[node.id] = node;
            reconcileEditorTabPlacements(state.nodes);
        }),

        updateNode: (id, patches) => set((state) => {
            const node = state.nodes[id];
            if (node) {
                Object.assign(node, patches);
            }
        }),

        splitEditorGroupAtEdge: (targetGroupId, edge, payload) => {
            let createdGroupId: string | null = null;
            set((state) => {
                if (payload.pinSourceActiveTab) {
                    const activeTab = getActiveLayoutTab(targetGroupId, state.nodes)?.tab;
                    if (activeTab) {
                        useEditorTabStore.getState().setTabPinned(targetGroupId, activeTab.id, true);
                    }
                }

                createdGroupId = splitEditorGroupInTree(state.nodes, targetGroupId, edge, {
                    component: payload.component,
                });
                if (createdGroupId) {
                    useEditorTabStore.getState().initGroupPlacement(
                        createdGroupId,
                        payload.tabs,
                        payload.activeTabId,
                    );
                    state.activeEditorGroupId = createdGroupId;
                }
            });
            return createdGroupId;
        },

        resizeNode: (nodeId, size, panelPosition) => set((state) => {
            const node = state.nodes[nodeId];
            if (node) {
                node.pixelSize = clampWorkbenchPartSize(
                    node,
                    size,
                    undefined,
                    panelPosition ?? inferPanelPosition(state.nodes),
                );
            }
        }),

        showSidebarTab: (tab) => set((state) => {
            const sidebar = state.nodes[SIDEBAR_NODE_ID];
            if (!sidebar) return;
            sidebar.data = { ...sidebar.data, visible: true, currentTab: tab };
        }),

        toggleSidebarTab: (tab) => set((state) => {
            const sidebar = state.nodes[SIDEBAR_NODE_ID];
            if (!sidebar) return;

            const isVisible = sidebar.data?.visible !== false;
            const activeTab = isSidebarTabId(sidebar.data?.currentTab) ? sidebar.data!.currentTab! : null;

            if (isVisible && activeTab === tab) {
                sidebar.data = { ...sidebar.data, visible: false };
                return;
            }

            sidebar.data = { ...sidebar.data, visible: true, currentTab: tab };
        }),

        collapseEditorGroups: () => set((state) => {
            const editorArea = state.nodes[EDITOR_AREA_ID];
            if (!editorArea?.children) return;

            const otherGroupIds = listEditorGroupIds(state.nodes).filter((id) => id !== DEFAULT_EDITOR_GROUP_ID);
            useEditorTabStore.getState().mergePlacementsIntoGroup(DEFAULT_EDITOR_GROUP_ID, otherGroupIds);

            for (const id of collectDescendantIds(state.nodes, EDITOR_AREA_ID, DEFAULT_EDITOR_GROUP_ID)) {
                delete state.nodes[id];
            }

            editorArea.children = [DEFAULT_EDITOR_GROUP_ID];

            writeEditorAreaMaximizeState(state.nodes, null, null);
            clearEditorGroupMaximizedHidden(state.nodes);

            const defaultEditor = state.nodes[DEFAULT_EDITOR_GROUP_ID];
            if (defaultEditor) {
                defaultEditor.parentId = EDITOR_AREA_ID;
                defaultEditor.size = 1;
                defaultEditor.pixelSize = undefined;
                defaultEditor.data = {
                    ...defaultEditor.data,
                    component: 'GraphEditor',
                };
            } else {
                state.nodes[DEFAULT_EDITOR_GROUP_ID] = {
                    id: DEFAULT_EDITOR_GROUP_ID,
                    type: 'component',
                    parentId: EDITOR_AREA_ID,
                    data: { component: 'GraphEditor' },
                };
            }

            reconcileEditorTabPlacements(state.nodes);
            state.activeEditorGroupId = DEFAULT_EDITOR_GROUP_ID;
        }),

        removeEditorGroup: (groupId) => {
            let removed = false;
            set((state) => {
                const result = removeEditorGroupFromTree(state.nodes, groupId);
                removed = result.removed;
                if (removed) {
                    useEditorTabStore.getState().removeGroupPlacement(groupId);
                    if (state.activeEditorGroupId === groupId) {
                        state.activeEditorGroupId = result.nextActiveGroupId;
                    }
                }
                ensureValidActiveEditorGroup(state);
            });
            return removed;
        },

        toggleMaximizeEditorGroup: (groupId) => set((state) => {
            const editorArea = state.nodes[EDITOR_AREA_ID];
            if (!editorArea || !state.nodes[groupId]) return;

            const current = readEditorAreaMaximizedGroupId(state.nodes);
            if (current === groupId) {
                const restored = readEditorAreaRestoredGridSizes(state.nodes);
                clearEditorGroupMaximizedHidden(state.nodes);
                writeEditorAreaMaximizeState(state.nodes, null, null);
                if (restored) applyEditorGridPixelSizes(state.nodes, restored);
                normalizeEditorGridSplitWeights(state.nodes);
                return;
            }

            const restoredGridSizes = snapshotEditorGridPixelSizes(state.nodes);
            writeEditorAreaMaximizeState(state.nodes, groupId, restoredGridSizes);

            for (const id of listEditorGroupIds(state.nodes)) {
                setEditorGroupMaximizedHidden(state.nodes, id, id !== groupId);
            }
            state.activeEditorGroupId = groupId;
        }),

        resetEditorGridSplitEqual: (beforeId, afterId, beforeSize, afterSize) => set((state) => {
            applyEqualGridSplit(state.nodes, beforeId, afterId, beforeSize, afterSize);
        }),

        resetWorkbenchLayout: () => set((state) => {
            const defaults = createInitialWorkbenchNodes();
            state.rootId = WORKBENCH_ROOT_ID;
            state.nodes[WORKBENCH_ROOT_ID] = defaults[WORKBENCH_ROOT_ID]!;
            state.nodes.center = defaults.center!;
            state.nodes.sidebar = defaults.sidebar!;
            state.nodes.panel = defaults.panel!;
            state.nodes.detail = defaults.detail!;
            state.zenMode = false;
            useEditorTabStore.getState().importFromLayoutNodes(state.nodes);
            useEditorTabStore.getState().stripEmbeddedTabsFromNodes(state.nodes);
            reconcileEditorTabPlacements(state.nodes);
        }),

        moveTab: (sourceNodeId, tabId, targetNodeId, targetTabIndex) => {
            const tabStore = useEditorTabStore.getState();
            if (!tabStore.locateTab(tabId, sourceNodeId)) {
                const legacyTab = readLegacyEmbeddedTab(get().nodes[sourceNodeId], tabId);
                if (legacyTab) {
                    tabStore.addTab(targetNodeId, legacyTab, targetTabIndex);
                    set((state) => {
                        removeLegacyEmbeddedTab(state.nodes[sourceNodeId], tabId);
                    });
                }
            } else {
                tabStore.moveTab(sourceNodeId, tabId, targetNodeId, targetTabIndex);
            }
            set((state) => {
                const sourceEmpty = isEditorGroupPlacementEmpty(sourceNodeId);
                if (sourceEmpty) {
                    removeEmptyEditorGroupIfNeeded(state, sourceNodeId, targetNodeId);
                }
                state.activeEditorGroupId = targetNodeId;
                ensureValidActiveEditorGroup(state);
            });
        },

        removeTab: (nodeId, tabId) => {
            useEditorTabStore.getState().removeTab(nodeId, tabId);
            set((state) => {
                if (isEditorGroupPlacementEmpty(nodeId)) {
                    if (listEditorGroupIds(state.nodes).length > 1) {
                        removeEmptyEditorGroupIfNeeded(state, nodeId);
                    }
                }
                ensureValidActiveEditorGroup(state);
            });
        },

        addTab: (nodeId, tab, insertIndex) => {
            useEditorTabStore.getState().addTab(nodeId, tab, insertIndex);
        },

        setTabPinned: (nodeId, tabId, pinned) => {
            useEditorTabStore.getState().setTabPinned(nodeId, tabId, pinned);
        },

        setEditorGroupActiveTab: (nodeId, tabId) => {
            useEditorTabStore.getState().setActiveTab(nodeId, tabId);
        },

        closeAllGraphTabs: () => {
            useEditorTabStore.getState().closeAllGraphTabs();
            set((state) => {
                for (const groupId of listEditorGroupIds(state.nodes)) {
                    if (isEditorGroupPlacementEmpty(groupId)) {
                        removeEmptyEditorGroupIfNeeded(state, groupId);
                    }
                }
                ensureValidActiveEditorGroup(state);
            });
        },

        isSettingsOpen: false,
        zenMode: false,
        openSettings: () => set((state) => {
            state.isSettingsOpen = true;
        }),
        closeSettings: () => set((state) => {
            state.isSettingsOpen = false;
        }),
        setSettingsOpen: (open) => set((state) => {
            state.isSettingsOpen = open;
        }),

        setDragging: (isDragging) => set((state) => {
            state.isDragging = isDragging;
        }),

        setActiveGroup: (id) => set((state) => {
            const node = id ? state.nodes[id] : null;
            if (node?.type === 'component' && !node.data?.isFixed) {
                state.activeEditorGroupId = id;
            }
        }),

        commitFlexSplitResize: (beforeId, afterId, beforeSize, afterSize) => set((state) => {
            commitSplitPairSizes(state.nodes, beforeId, afterId, beforeSize, afterSize);
        }),

        isAltPressed: false,
        setAltPressed: (pressed) => set((state) => {
            if (state.isAltPressed !== pressed) {
                state.isAltPressed = pressed;
            }
        }),
    }))
);

// Bootstrap tab placements from initial layout nodes on module load.
const initialNodes = useLayoutStore.getState().nodes;
useEditorTabStore.getState().importFromLayoutNodes(initialNodes);
useEditorTabStore.getState().stripEmbeddedTabsFromNodes(initialNodes);
reconcileEditorTabPlacements(initialNodes);
