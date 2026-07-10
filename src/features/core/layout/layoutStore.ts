import { create } from 'zustand';
import { immer } from 'zustand/middleware/immer';
import { LayoutNode, LayoutTree, LayoutDirection, LayoutTab } from '@/shared/types/ui';
import { getActiveLayoutTab } from './layoutTabQueries';
import type { EditorSplitEdge } from './editorSplitLayout';
import { clampWorkbenchPartSize } from './workbenchPanelSizing';
import {
    createInitialWorkbenchNodes,
    DEFAULT_EDITOR_GROUP_ID,
    EDITOR_AREA_ID,
    WORKBENCH_ROOT_ID,
    WORKBENCH_PART_IDS,
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

function clearSelectedNodeIds(data: LayoutNode['data']): LayoutNode['data'] {
    return {
        ...data,
        params: {
            ...data?.params,
            selectedNodeIds: [],
        },
    };
}

export interface LayoutState {
    rootId: string;
    nodes: LayoutTree;

    // Actions
    getNode: (id: string) => LayoutNode | undefined;

    // Basic tree manipulation
    addNode: (node: LayoutNode) => void;
    updateNode: (id: string, patches: Partial<LayoutNode>) => void;
    removeNode: (id: string) => void;

    // High level actions
    splitNode: (targetId: string, direction: LayoutDirection, newComponentType: string) => void;
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
    resizeNode: (nodeId: string, size: number) => void;

    /** Show sidebar and switch activity tab. pixelSize is preserved while hidden. */
    showSidebarTab: (tab: SidebarTabId) => void;
    /** Toggle sidebar visibility; same tab click hides, different tab switches. */
    toggleSidebarTab: (tab: SidebarTabId) => void;

    /** Collapse split editor groups back to a single default group. */
    collapseEditorGroups: () => void;
    /** Reset workbench chrome + editor grid to initial layout. */
    resetWorkbenchLayout: () => void;

    // DND Actions
    moveTab: (sourceNodeId: string, tabId: string, targetNodeId: string, targetTabIndex?: number) => void;
    removeTab: (nodeId: string, tabId: string) => void;
    addTab: (nodeId: string, tab: LayoutTab) => void;
    setTabPinned: (nodeId: string, tabId: string, pinned: boolean) => void;
    /** Patch only activeTabId (+ clear selection on change). Avoids full node spread on tab switch. */
    setEditorGroupActiveTab: (nodeId: string, tabId: string | null) => void;
    /** Pixelize both flex siblings after first sash drag between editor groups. */
    commitFlexSplitResize: (beforeId: string, afterId: string, beforeSize: number, afterSize: number) => void;
    /** VS Code maximize editor group — hide sibling groups until toggled off. */
    toggleMaximizeEditorGroup: (groupId: string) => void;
    /** Reset a flex/pixel split between two editor-grid siblings to equal sizes. */
    resetEditorGridSplitEqual: (beforeId: string, afterId: string, beforeSize: number, afterSize: number) => void;
    /**
     * Drop every graph (event/function) tab from every editor group, regardless
     * of dirty state. Use during destructive project transitions (load / clear /
     * switch) where the previous project's graphs are no longer valid.
     */
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

function snapshotFixedChromeSizes(nodes: LayoutTree): Record<string, number | undefined> {
    const sizes: Record<string, number | undefined> = {};
    for (const id of WORKBENCH_PART_IDS) {
        sizes[id] = nodes[id]?.pixelSize;
    }
    return sizes;
}

function restoreFixedChromeSizes(state: { nodes: LayoutTree }, saved: Record<string, number | undefined>): void {
    for (const id of WORKBENCH_PART_IDS) {
        const savedSize = saved[id];
        if (savedSize === undefined) continue;
        const node = state.nodes[id];
        if (node) {
            node.pixelSize = savedSize;
        }
    }
}

function removeEmptyEditorGroupIfNeeded(
    state: { nodes: LayoutTree; activeEditorGroupId: string | null },
    groupId: string,
    preferActiveGroupId?: string,
): void {
    const { removed, nextActiveGroupId } = removeEditorGroupFromTree(state.nodes, groupId);
    if (!removed) return;
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
        }),

        updateNode: (id, patches) => set((state) => {
            const node = state.nodes[id];
            if (node) {
                Object.assign(node, patches);
            }
        }),

        removeNode: (id) => set((state) => {
            const node = state.nodes[id];
            if (!node || !node.parentId) return;

            // 识别编辑器组（非固定组件）
            const isEditor = node.type === 'component' && !node.data?.isFixed;
            if (isEditor) {
                const editorGroups = Object.values(state.nodes).filter(n => n.type === 'component' && !n.data?.isFixed);
                if (editorGroups.length <= 1) {
                    // 如果是最后一个编辑器组，不执行删除，仅清空其内容
                    node.data = {
                        ...node.data,
                        tabs: [],
                        activeTabId: undefined
                    };
                    return;
                }
            }

            const parent = state.nodes[node.parentId];
            if (parent && parent.children) {
                parent.children = parent.children.filter(childId => childId !== id);

                // 如果父容器只剩一个子节点，提升该子节点以替代父容器，避免 pixelSize 导致空白
                if (parent.children.length === 1 && parent.parentId) {
                    const grandParent = state.nodes[parent.parentId];
                    if (grandParent?.children) {
                        const singleChildId = parent.children[0];
                        const singleChild = state.nodes[singleChildId];
                        if (singleChild) {
                            const parentIndex = grandParent.children.indexOf(parent.id);
                            grandParent.children[parentIndex] = singleChildId;
                            singleChild.parentId = grandParent.id;
                            singleChild.size = parent.size ?? 1;
                            singleChild.pixelSize = undefined; // 清除固定尺寸，让其填满空间
                            delete state.nodes[parent.id];
                        }
                    }
                } else if (parent.children.length === 0 && parent.parentId) {
                    const grandParent = state.nodes[parent.parentId];
                    if (grandParent?.children) {
                        grandParent.children = grandParent.children.filter(cid => cid !== parent.id);
                        delete state.nodes[parent.id];
                    }
                }
            }
            delete state.nodes[id];

            // 自动重设焦点到剩余的编辑器组
            const remainingEditors = Object.values(state.nodes).filter(n => n.type === 'component' && !n.data?.isFixed);
            if (state.activeEditorGroupId === id) {
                state.activeEditorGroupId = remainingEditors[0]?.id || null;
            }

            // 最后验证：确保激活的编辑器存在且有效
            const activeNode = state.nodes[state.activeEditorGroupId || ''];
            if (!activeNode || activeNode.type !== 'component' || activeNode.data?.isFixed) {
                if (remainingEditors.length > 0) {
                    state.activeEditorGroupId = remainingEditors[0].id;
                }
            }
        }),

        splitNode: (targetId, direction, newComponentType) => {
            const activeTab = getActiveLayoutTab(targetId, get().nodes)?.tab;
            get().splitEditorGroupAtEdge(targetId, direction === 'row' ? 'right' : 'bottom', {
                component: newComponentType,
                tabs: activeTab ? [{ ...activeTab, pinned: true as const }] : [],
                activeTabId: activeTab?.id,
                pinSourceActiveTab: true,
            });
        },

        splitEditorGroupAtEdge: (targetGroupId, edge, payload) => {
            let createdGroupId: string | null = null;
            set((state) => {
                if (payload.pinSourceActiveTab) {
                    const activeTab = getActiveLayoutTab(targetGroupId, state.nodes)?.tab;
                    if (activeTab) {
                        const targetNode = state.nodes[targetGroupId];
                        const sourceTab = targetNode?.data?.tabs?.find((t) => t.id === activeTab.id);
                        if (sourceTab) sourceTab.pinned = true;
                    }
                }

                createdGroupId = splitEditorGroupInTree(state.nodes, targetGroupId, edge, {
                    component: payload.component,
                    tabs: payload.tabs,
                    activeTabId: payload.activeTabId,
                });
                if (createdGroupId) {
                    state.activeEditorGroupId = createdGroupId;
                }
            });
            return createdGroupId;
        },

        resizeNode: (nodeId, size) => set((state) => {
            const node = state.nodes[nodeId];
            if (node) {
                node.pixelSize = clampWorkbenchPartSize(node, size);
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

            const defaultEditor = state.nodes[DEFAULT_EDITOR_GROUP_ID];
            const tabs = defaultEditor?.data?.tabs ?? [];
            const activeTabId = defaultEditor?.data?.activeTabId;

            for (const id of collectDescendantIds(state.nodes, EDITOR_AREA_ID, DEFAULT_EDITOR_GROUP_ID)) {
                delete state.nodes[id];
            }

            editorArea.children = [DEFAULT_EDITOR_GROUP_ID];

            writeEditorAreaMaximizeState(state.nodes, null, null);
            clearEditorGroupMaximizedHidden(state.nodes);

            if (defaultEditor) {
                defaultEditor.parentId = EDITOR_AREA_ID;
                defaultEditor.size = 1;
                defaultEditor.pixelSize = undefined;
                defaultEditor.data = {
                    ...defaultEditor.data,
                    component: 'GraphEditor',
                    tabs,
                    activeTabId,
                };
            } else {
                state.nodes[DEFAULT_EDITOR_GROUP_ID] = {
                    id: DEFAULT_EDITOR_GROUP_ID,
                    type: 'component',
                    parentId: EDITOR_AREA_ID,
                    data: { component: 'GraphEditor', tabs: [] },
                };
            }

            state.activeEditorGroupId = DEFAULT_EDITOR_GROUP_ID;
        }),

        toggleMaximizeEditorGroup: (groupId) => set((state) => {
            const editorArea = state.nodes[EDITOR_AREA_ID];
            if (!editorArea || !state.nodes[groupId]) return;

            const current = readEditorAreaMaximizedGroupId(state.nodes);
            if (current === groupId) {
                const restored = readEditorAreaRestoredGridSizes(state.nodes);
                clearEditorGroupMaximizedHidden(state.nodes);
                if (restored) applyEditorGridPixelSizes(state.nodes, restored);
                writeEditorAreaMaximizeState(state.nodes, null, null);
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
            state.rootId = WORKBENCH_ROOT_ID;
            state.nodes = createInitialWorkbenchNodes();
            state.activeEditorGroupId = DEFAULT_EDITOR_GROUP_ID;
        }),

        moveTab: (sourceNodeId, tabId, targetNodeId, targetTabIndex) => set((state) => {
            const sourceNode = state.nodes[sourceNodeId];
            const targetNode = state.nodes[targetNodeId];
            if (!sourceNode || !targetNode) return;

            const sourceTabs = sourceNode.data?.tabs || [];
            const tabToMove = sourceTabs.find(t => t.id === tabId);
            if (!tabToMove) return;
            tabToMove.pinned = true;

            // 检查目标节点是否已经有这个标签页
            const targetTabs = targetNode.data?.tabs || [];
            const existingTabIndex = targetTabs.findIndex(t => t.id === tabId);

            // 如果目标节点已经有这个标签页，只激活它
            if (existingTabIndex !== -1 && sourceNodeId !== targetNodeId) {
                // 从源节点移除
                sourceNode.data!.tabs = sourceTabs.filter(t => t.id !== tabId);
                if (sourceNode.data!.activeTabId === tabId) {
                    const closingIndex = sourceTabs.findIndex(t => t.id === tabId);
                    const nextIndex = Math.max(0, closingIndex - 1);
                    sourceNode.data!.activeTabId = sourceNode.data!.tabs[nextIndex]?.id;
                }

                // 清理空的源节点
                if (sourceNode.data!.tabs.length === 0) {
                    removeEmptyEditorGroupIfNeeded(state, sourceNodeId, targetNodeId);
                }

                // 只激活目标节点中已存在的标签页
                targetNode.data!.activeTabId = tabId;
                // 确保目标编辑器被激活
                state.activeEditorGroupId = targetNodeId;
                return;
            }

            // 如果是同一个节点内部移动，只做重新排序
            if (sourceNodeId === targetNodeId) {
                const currentIndex = sourceTabs.findIndex(t => t.id === tabId);
                if (currentIndex === -1) return;

                const newTabs = [...sourceTabs];
                const [removed] = newTabs.splice(currentIndex, 1);

                if (targetTabIndex !== undefined) {
                    // 如果目标索引大于当前索引，需要减 1，因为已经移除了一个元素
                    const adjustedIndex = targetTabIndex > currentIndex ? targetTabIndex - 1 : targetTabIndex;
                    newTabs.splice(adjustedIndex, 0, removed);
                } else {
                    newTabs.push(removed);
                }

                sourceNode.data!.tabs = newTabs;
                sourceNode.data!.activeTabId = tabId;
                return;
            }

            // 正常的跨节点移动：从源节点移除
            const closingIndex = sourceTabs.findIndex(t => t.id === tabId);
            sourceNode.data!.tabs = sourceTabs.filter(t => t.id !== tabId);
            if (sourceNode.data!.activeTabId === tabId) {
                const nextIndex = Math.max(0, closingIndex - 1);
                sourceNode.data!.activeTabId = sourceNode.data!.tabs[nextIndex]?.id;
            }

            // VS Code 逻辑：如果源节点没有 tabs 了，且不是最后一个编辑器组，移除源节点
            if (sourceNode.data!.tabs.length === 0) {
                removeEmptyEditorGroupIfNeeded(state, sourceNodeId, targetNodeId);
            }

            // 添加到目标节点（此时已确认目标节点没有这个标签页）
            if (targetTabIndex !== undefined) {
                targetTabs.splice(targetTabIndex, 0, tabToMove);
            } else {
                targetTabs.push(tabToMove);
            }
            targetNode.data = {
                ...targetNode.data,
                tabs: targetTabs,
                activeTabId: tabId
            };

            // 确保目标编辑器被激活
            state.activeEditorGroupId = targetNodeId;
            ensureValidActiveEditorGroup(state);
        }),

        removeTab: (nodeId, tabId) => set((state) => {
            const savedChromeSizes = snapshotFixedChromeSizes(state.nodes);
            const node = state.nodes[nodeId];
            if (!node || !node.data?.tabs) return;

            const currentTabs = node.data.tabs;
            const closingIndex = currentTabs.findIndex(t => t.id === tabId);
            if (closingIndex === -1) return;

            const newTabs = currentTabs.filter(t => t.id !== tabId);

            if (newTabs.length === 0) {
                if (listEditorGroupIds(state.nodes).length > 1) {
                    const { removed, nextActiveGroupId } = removeEditorGroupFromTree(state.nodes, nodeId);
                    if (removed) {
                        state.activeEditorGroupId = nextActiveGroupId;
                    }
                } else {
                    // 最后一个组，保留但清空
                    node.data.tabs = [];
                    node.data.activeTabId = undefined;
                }
            } else {
                const data = node.data!;
                // 还有剩余标签，处理激活状态
                let newActiveTabId = data.activeTabId;
                if (newActiveTabId === tabId) {
                    const nextIndex = Math.max(0, closingIndex - 1);
                    newActiveTabId = newTabs[nextIndex]?.id;
                }
                data.tabs = newTabs;
                if (data.activeTabId !== newActiveTabId) {
                    node.data = clearSelectedNodeIds(data);
                }
                node.data!.activeTabId = newActiveTabId;
            }

            ensureValidActiveEditorGroup(state);
            restoreFixedChromeSizes(state, savedChromeSizes);
        }),

        addTab: (nodeId, tab) => set((state) => {
            const node = state.nodes[nodeId];
            if (!node || node.type !== 'component') return;

            const tabs = node.data?.tabs || [];
            // 如果标签已存在，则激活它
            if (tabs.find(t => t.id === tab.id)) {
                if (node.data!.activeTabId !== tab.id) {
                    node.data = clearSelectedNodeIds(node.data);
                }
                node.data!.activeTabId = tab.id;
                return;
            }

            // 添加新标签
            node.data = {
                ...clearSelectedNodeIds(node.data),
                tabs: [...tabs, tab],
                activeTabId: tab.id,
                component: node.data?.component || 'GraphEditor'
            };
        }),

        setTabPinned: (nodeId, tabId, pinned) => set((state) => {
            const tab = state.nodes[nodeId]?.data?.tabs?.find((item) => item.id === tabId);
            if (!tab) return;
            tab.pinned = pinned;
        }),

        setEditorGroupActiveTab: (nodeId, tabId) => set((state) => {
            const node = state.nodes[nodeId];
            if (!node?.data?.tabs) return;

            const nextActiveId = tabId || undefined;
            if (node.data.activeTabId === nextActiveId) return;

            node.data.activeTabId = nextActiveId;
            node.data.params = {
                ...node.data.params,
                selectedNodeIds: [],
            };
        }),

        closeAllGraphTabs: () => set((state) => {
            for (const node of Object.values(state.nodes)) {
                if (node.type !== 'component' || !node.data?.tabs) continue;
                const remaining = node.data.tabs.filter(
                    (tab) => tab.type !== 'event' && tab.type !== 'function' && tab.type !== 'worksheet'
                );
                if (remaining.length === node.data.tabs.length) continue;
                const activeStillPresent = remaining.some((tab) => tab.id === node.data?.activeTabId);
                node.data.tabs = remaining;
                node.data.activeTabId = activeStillPresent
                    ? node.data?.activeTabId
                    : remaining[remaining.length - 1]?.id;
            }
        }),

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
            const before = state.nodes[beforeId];
            const after = state.nodes[afterId];
            if (before) before.pixelSize = beforeSize;
            if (after) after.pixelSize = afterSize;
        }),

        isAltPressed: false,
        setAltPressed: (pressed) => set((state) => {
            if (state.isAltPressed !== pressed) {
                state.isAltPressed = pressed;
            }
        }),
    }))
);
