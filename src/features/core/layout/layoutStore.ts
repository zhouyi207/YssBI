import { create } from 'zustand';
import { immer } from 'zustand/middleware/immer';
import { LayoutNode, LayoutTree, LayoutDirection, LayoutTab } from '@/shared/types/ui';
import { getActiveLayoutTab } from './layoutTabQueries';
import {
    createEditorGroupId,
    resolveEditorSplitPlacement,
    type EditorSplitEdge,
} from './editorSplitLayout';
import { logger } from '@/utils/appLogger';

// Helper to generate IDs
const generateId = () => Math.random().toString(36).slice(2, 11);

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

    // DND Actions
    moveNode: (sourceId: string, targetId: string, position: 'center' | 'top' | 'bottom' | 'left' | 'right') => void;
    moveTab: (sourceNodeId: string, tabId: string, targetNodeId: string, targetTabIndex?: number) => void;
    removeTab: (nodeId: string, tabId: string) => void;
    addTab: (nodeId: string, tab: LayoutTab) => void;
    setTabPinned: (nodeId: string, tabId: string, pinned: boolean) => void;
    /**
     * Drop every graph (event/function) tab from every editor group, regardless
     * of dirty state. Use during destructive project transitions (load / clear /
     * switch) where the previous project's graphs are no longer valid.
     */
    closeAllGraphTabs: () => void;

    // UI State
    isDragging: boolean;
    setDragging: (isDragging: boolean) => void;
    activeGroupId: string | null;
    activeEditorGroupId: string | null;
    isSettingsOpen: boolean;
    setActiveGroup: (id: string | null) => void;
    openSettings: () => void;
    closeSettings: () => void;
    setSettingsOpen: (open: boolean) => void;
    isAltPressed: boolean;
    setAltPressed: (pressed: boolean) => void;
}

// Initial Layout Structure (VSCode-style: sidebar | center(editor+panel) | detail)
const INITIAL_ROOT_ID = 'root';
const FIXED_CHROME_NODE_IDS = ['sidebar', 'detail', 'panel'] as const;
function snapshotFixedChromeSizes(nodes: LayoutTree): Record<string, number | undefined> {
    const sizes: Record<string, number | undefined> = {};
    for (const id of FIXED_CHROME_NODE_IDS) {
        sizes[id] = nodes[id]?.pixelSize;
    }
    return sizes;
}

function restoreFixedChromeSizes(state: { nodes: LayoutTree }, saved: Record<string, number | undefined>): void {
    for (const id of FIXED_CHROME_NODE_IDS) {
        const savedSize = saved[id];
        if (savedSize === undefined) continue;
        const node = state.nodes[id];
        if (node) {
            node.pixelSize = savedSize;
        }
    }
}

const INITIAL_NODES: LayoutTree = {
    [INITIAL_ROOT_ID]: {
        id: INITIAL_ROOT_ID,
        type: 'row',
        parentId: null,
        children: ['sidebar', 'center', 'detail'],
    },
    'sidebar': {
        id: 'sidebar',
        type: 'component',
        parentId: INITIAL_ROOT_ID,
        pixelSize: 260,
        minSize: 240,
        data: { component: 'Sidebar', visible: true, title: 'Explorer', isFixed: true, currentTab: 'graphs' },
    },
    'center': {
        id: 'center',
        type: 'col',
        parentId: INITIAL_ROOT_ID,
        children: ['editor_area', 'panel'],
        size: 1,
    },
    'editor_area': {
        id: 'editor_area',
        type: 'row',
        parentId: 'center',
        children: ['default_editor'],
        size: 1,
    },
    'default_editor': {
        id: 'default_editor',
        type: 'component',
        parentId: 'editor_area',
        data: {
            component: 'GraphEditor',
            tabs: []
        },
    },
    'panel': {
        id: 'panel',
        type: 'component',
        parentId: 'center',
        pixelSize: 200,
        minSize: 80,
        data: { component: 'LogPanel', visible: true, title: 'Logs', isFixed: true },
    },
    'detail': {
        id: 'detail',
        type: 'component',
        parentId: INITIAL_ROOT_ID,
        pixelSize: 300,
        minSize: 240,
        data: { component: 'Detail', visible: true, title: 'Properties', isFixed: true },
    }
};

export const useLayoutStore = create<LayoutState>()(
    immer((set, get) => ({
        rootId: INITIAL_ROOT_ID,
        nodes: INITIAL_NODES,
        isDragging: false,
        activeGroupId: 'default_editor', // Default focus
        activeEditorGroupId: 'default_editor',

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
            if (state.activeGroupId === id) {
                state.activeGroupId = remainingEditors[0]?.id || null;
            }
            if (state.activeEditorGroupId === id) {
                state.activeEditorGroupId = remainingEditors[0]?.id || null;
            }

            // 最后验证：确保激活的编辑器存在且有效
            const activeNode = state.nodes[state.activeEditorGroupId || ''];
            if (!activeNode || activeNode.type !== 'component' || activeNode.data?.isFixed) {
                if (remainingEditors.length > 0) {
                    state.activeGroupId = remainingEditors[0].id;
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
                const { direction, isAfter } = resolveEditorSplitPlacement(edge);
                const targetNode = state.nodes[targetGroupId];
                if (!targetNode || !targetNode.parentId) return;

                const parentNode = state.nodes[targetNode.parentId];
                if (!parentNode) return;

                if (payload.pinSourceActiveTab) {
                    const activeTab = getActiveLayoutTab(targetGroupId, state.nodes)?.tab;
                    if (activeTab) {
                        const sourceTab = targetNode.data?.tabs?.find((t) => t.id === activeTab.id);
                        if (sourceTab) sourceTab.pinned = true;
                    }
                }

                const newNodeId = createEditorGroupId();
                const newNode: LayoutNode = {
                    id: newNodeId,
                    type: 'component',
                    parentId: parentNode.id,
                    children: [],
                    size: 1,
                    data: {
                        component: payload.component,
                        tabs: payload.tabs,
                        activeTabId: payload.activeTabId,
                    },
                };

                if (parentNode.type === direction) {
                    const targetIndex = parentNode.children?.indexOf(targetGroupId) ?? 0;
                    const insertIndex = isAfter ? targetIndex + 1 : targetIndex;
                    parentNode.children?.splice(insertIndex, 0, newNodeId);
                    state.nodes[newNodeId] = newNode;
                } else {
                    const branchId = createEditorGroupId();
                    const branch: LayoutNode = {
                        id: branchId,
                        type: direction,
                        parentId: parentNode.id,
                        children: isAfter ? [targetGroupId, newNodeId] : [newNodeId, targetGroupId],
                        size: targetNode.size,
                        pixelSize: targetNode.pixelSize,
                    };

                    const targetIndex = parentNode.children?.indexOf(targetGroupId) ?? 0;
                    parentNode.children![targetIndex] = branchId;

                    targetNode.parentId = branchId;
                    targetNode.size = 1;
                    targetNode.pixelSize = undefined;

                    newNode.parentId = branchId;

                    state.nodes[newNodeId] = newNode;
                    state.nodes[branchId] = branch;
                }

                state.activeGroupId = newNodeId;
                state.activeEditorGroupId = newNodeId;
                createdGroupId = newNodeId;
            });
            return createdGroupId;
        },

        resizeNode: (nodeId, size) => set((state) => {
            const node = state.nodes[nodeId];
            if (node) {
                node.pixelSize = size;
            }
        }),

        moveNode: (sourceId, targetId, position) => set((state) => {
            logger.sys.trace('Moving node ' + sourceId + ' to ' + targetId + ' ' + position, 'LayoutStore');
            const sourceNode = state.nodes[sourceId];
            const targetNode = state.nodes[targetId];
            if (!sourceNode || !targetNode || sourceId === targetId) return;

            // 如果是中心区域停靠，执行合并逻辑
            if (position === 'center') {
                if (sourceNode.type === 'component' && targetNode.type === 'component') {
                    const sourceTabs = sourceNode.data?.tabs || [];
                    const targetTabs = targetNode.data?.tabs || [];

                    // 合并 tabs
                    targetNode.data = {
                        ...targetNode.data,
                        tabs: [...targetTabs, ...sourceTabs],
                        activeTabId: sourceNode.data?.activeTabId || targetNode.data?.activeTabId
                    };

                    // 从父节点移除源节点
                    const sourceParent = state.nodes[sourceNode.parentId!];
                    if (sourceParent && sourceParent.children) {
                        sourceParent.children = sourceParent.children.filter(id => id !== sourceId);
                        if (sourceParent.children.length === 1 && sourceParent.parentId) {
                            const grandParent = state.nodes[sourceParent.parentId];
                            if (grandParent?.children) {
                                const singleChildId = sourceParent.children[0];
                                const singleChild = state.nodes[singleChildId];
                                if (singleChild) {
                                    const parentIndex = grandParent.children.indexOf(sourceParent.id);
                                    grandParent.children[parentIndex] = singleChildId;
                                    singleChild.parentId = grandParent.id;
                                    singleChild.size = sourceParent.size ?? 1;
                                    singleChild.pixelSize = undefined;
                                    delete state.nodes[sourceParent.id];
                                }
                            }
                        } else if (sourceParent.children.length === 0 && sourceParent.parentId) {
                            const grandParent = state.nodes[sourceParent.parentId];
                            if (grandParent?.children) {
                                grandParent.children = grandParent.children.filter(cid => cid !== sourceParent.id);
                                delete state.nodes[sourceParent.id];
                            }
                        }
                    }
                    delete state.nodes[sourceId];
                    return;
                }
                return;
            }

            const sourceParentId = sourceNode.parentId;
            const targetParentId = targetNode.parentId;
            if (!sourceParentId || !targetParentId) return;

            // 从原父节点移除
            const sourceParent = state.nodes[sourceParentId];
            if (sourceParent.children) {
                sourceParent.children = sourceParent.children.filter(id => id !== sourceId);
            }

            // 计算新节点的布局方向
            const direction: 'row' | 'col' = (position === 'left' || position === 'right') ? 'row' : 'col';
            const isAfter = position === 'right' || position === 'bottom';

            const targetParent = state.nodes[targetParentId];
            if (targetParent.type === direction) {
                // 如果父节点方向一致，直接插入
                const targetIndex = targetParent.children?.indexOf(targetId) || 0;
                targetParent.children?.splice(isAfter ? targetIndex + 1 : targetIndex, 0, sourceId);
                sourceNode.parentId = targetParentId;
            } else {
                // 否则需要创建新的分支节点
                const branchId = generateId();
                const branch: LayoutNode = {
                    id: branchId,
                    type: direction,
                    parentId: targetParentId,
                    children: isAfter ? [targetId, sourceId] : [sourceId, targetId],
                    size: targetNode.size,
                    pixelSize: targetNode.pixelSize
                };

                const targetIndex = targetParent.children?.indexOf(targetId) || 0;
                targetParent.children![targetIndex] = branchId;

                targetNode.parentId = branchId;
                targetNode.size = 1;
                targetNode.pixelSize = undefined;

                sourceNode.parentId = branchId;
                sourceNode.size = 1;
                sourceNode.pixelSize = undefined;

                state.nodes[branchId] = branch;
            }
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
                    const editorGroups = Object.values(state.nodes).filter(n => n.type === 'component' && !n.data?.isFixed);
                    if (editorGroups.length > 1) {
                        const parent = state.nodes[sourceNode.parentId!];
                        if (parent && parent.children) {
                            parent.children = parent.children.filter(id => id !== sourceNodeId);
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
                                        singleChild.pixelSize = undefined;
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
                        delete state.nodes[sourceNodeId];

                        // 如果删除的是当前激活的编辑器，切换到目标编辑器
                        if (state.activeGroupId === sourceNodeId) {
                            state.activeGroupId = targetNodeId;
                            state.activeEditorGroupId = targetNodeId;
                        }
                        if (state.activeEditorGroupId === sourceNodeId) {
                            state.activeEditorGroupId = targetNodeId;
                        }
                    }
                }

                // 只激活目标节点中已存在的标签页
                targetNode.data!.activeTabId = tabId;
                // 确保目标编辑器被激活
                state.activeGroupId = targetNodeId;
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
                const editorGroups = Object.values(state.nodes).filter(n => n.type === 'component' && !n.data?.isFixed);
                if (editorGroups.length > 1) {
                    const parent = state.nodes[sourceNode.parentId!];
                    if (parent && parent.children) {
                        parent.children = parent.children.filter(id => id !== sourceNodeId);
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
                                    singleChild.pixelSize = undefined;
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
                    delete state.nodes[sourceNodeId];

                    // 如果删除的是当前激活的编辑器，切换到目标编辑器
                    if (state.activeGroupId === sourceNodeId) {
                        state.activeGroupId = targetNodeId;
                        state.activeEditorGroupId = targetNodeId;
                    }
                    if (state.activeEditorGroupId === sourceNodeId) {
                        state.activeEditorGroupId = targetNodeId;
                    }
                }
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
            state.activeGroupId = targetNodeId;
            state.activeEditorGroupId = targetNodeId;

            // 最后验证：确保激活的编辑器存在且有效
            const activeNode = state.nodes[state.activeEditorGroupId || ''];
            if (!activeNode || activeNode.type !== 'component' || activeNode.data?.isFixed) {
                const editorGroups = Object.values(state.nodes).filter(n => n.type === 'component' && !n.data?.isFixed);
                if (editorGroups.length > 0) {
                    state.activeGroupId = editorGroups[0].id;
                    state.activeEditorGroupId = editorGroups[0].id;
                }
            }
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
                // 如果是最后一个标签
                const editorGroups = Object.values(state.nodes).filter(n => n.type === 'component' && !n.data?.isFixed);

                if (editorGroups.length > 1) {
                    // 如果有多个组，移除该组
                    const parent = state.nodes[node.parentId!];
                    if (parent && parent.children) {
                        parent.children = parent.children.filter(id => id !== nodeId);

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
                                    singleChild.pixelSize = undefined;
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
                    delete state.nodes[nodeId];

                    // 重设焦点
                    const remainingEditors = Object.values(state.nodes).filter(n => n.type === 'component' && !n.data?.isFixed);
                    state.activeGroupId = remainingEditors[0]?.id || null;
                    state.activeEditorGroupId = remainingEditors[0]?.id || null;
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

            // 最后验证：确保激活的编辑器存在且有效
            const activeNode = state.nodes[state.activeEditorGroupId || ''];
            if (!activeNode || activeNode.type !== 'component' || activeNode.data?.isFixed) {
                const editorGroups = Object.values(state.nodes).filter(n => n.type === 'component' && !n.data?.isFixed);
                if (editorGroups.length > 0) {
                    state.activeGroupId = editorGroups[0].id;
                    state.activeEditorGroupId = editorGroups[0].id;
                }
            }

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
            state.activeGroupId = id;

            // 逻辑补充：如果该节点是非固定组件（编辑器组），则更新 activeEditorGroupId
            const node = id ? state.nodes[id] : null;
            if (node?.type === 'component' && !node.data?.isFixed) {
                state.activeEditorGroupId = id;
            }
        }),

        isAltPressed: false,
        setAltPressed: (pressed) => set((state) => {
            if (state.isAltPressed !== pressed) {
                state.isAltPressed = pressed;
            }
        }),
    }))
);
