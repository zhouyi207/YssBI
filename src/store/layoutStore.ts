import { create } from 'zustand';
import { immer } from 'zustand/middleware/immer';
import { LayoutNode, LayoutTree, LayoutDirection } from '../types/layout';

// Helper to generate IDs
const generateId = () => Math.random().toString(36).slice(2, 11);

interface LayoutState {
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
    resizeNode: (nodeId: string, size: number) => void;

    // DND Actions
    moveNode: (sourceId: string, targetId: string, position: 'center' | 'top' | 'bottom' | 'left' | 'right') => void;
    moveTab: (sourceNodeId: string, tabId: string, targetNodeId: string, targetTabIndex?: number) => void;
    addTab: (nodeId: string, tab: { id: string, title: string, component: string, params?: any }) => void;

    // UI State
    isDragging: boolean;
    setDragging: (isDragging: boolean) => void;
    activeGroupId: string | null;
    activeEditorGroupId: string | null;
    setActiveGroup: (id: string | null) => void;
}

// Initial Layout Structure
const INITIAL_ROOT_ID = 'root';
const INITIAL_NODES: LayoutTree = {
    [INITIAL_ROOT_ID]: {
        id: INITIAL_ROOT_ID,
        type: 'row',
        parentId: null,
        children: ['sidebar', 'main', 'detail'],
    },
    'sidebar': {
        id: 'sidebar',
        type: 'component',
        parentId: INITIAL_ROOT_ID,
        pixelSize: 260, // Default width
        minSize: 0,     // Allow collapsing to 0
        data: { component: 'Sidebar', visible: true, title: 'Explorer', isFixed: true, currentTab: 'variables' },
    },
    'main': {
        id: 'main',
        type: 'row',
        parentId: INITIAL_ROOT_ID,
        children: ['editor1', 'editor2'],
        size: 1, // Flex grow
    },
    'editor1': {
        id: 'editor1',
        type: 'component',
        parentId: 'main',
        data: { 
            component: 'GraphEditor',
            activeTabId: 'graph_a',
            tabs: [
                { id: 'graph_a', title: 'Main Graph', component: 'GraphEditor' }
            ]
        },
    },
    'editor2': {
        id: 'editor2',
        type: 'component',
        parentId: 'main',
        data: { 
            component: 'GraphEditor',
            activeTabId: 'graph_b',
            tabs: [
                { id: 'graph_b', title: 'Secondary Graph', component: 'GraphEditor' }
            ]
        },
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
        activeGroupId: 'editor1', // Default focus
        activeEditorGroupId: 'editor1',

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

            const parent = state.nodes[node.parentId];
            if (parent && parent.children) {
                parent.children = parent.children.filter(childId => childId !== id);
                
                // 如果父容器变空了（且不是根节点），递归删除父容器
                if (parent.children.length === 0 && parent.parentId) {
                    // 这里可以调用自身逻辑，但由于是 immer，直接操作 state 即可
                    const grandParent = state.nodes[parent.parentId];
                    if (grandParent && grandParent.children) {
                        grandParent.children = grandParent.children.filter(cid => cid !== parent.id);
                        delete state.nodes[parent.id];
                    }
                }
            }
            delete state.nodes[id];

            // 自动重设焦点
            const remainingEditors = Object.values(state.nodes).filter(n => n.type === 'component' && n.data?.tabs);
            if (state.activeGroupId === id) {
                state.activeGroupId = remainingEditors[0]?.id || null;
            }
            if (state.activeEditorGroupId === id) {
                state.activeEditorGroupId = remainingEditors[0]?.id || null;
            }
        }),

        splitNode: (targetId, direction, newComponentType) => set((state) => {
            const targetNode = state.nodes[targetId];
            if (!targetNode || !targetNode.parentId) return;

            const parentNode = state.nodes[targetNode.parentId];
            const requiredDirection = direction;

            const newNodeId = generateId();
            const newNode: LayoutNode = {
                id: newNodeId,
                type: 'component',
                parentId: parentNode.id,
                children: [],
                size: 1,
                data: { component: newComponentType }
            };

            if (parentNode.type === requiredDirection) {
                const targetIndex = parentNode.children?.indexOf(targetId) || 0;
                parentNode.children?.splice(targetIndex + 1, 0, newNodeId);
                state.nodes[newNodeId] = newNode;
            } else {
                const branchId = generateId();
                const branch: LayoutNode = {
                    id: branchId,
                    type: requiredDirection,
                    parentId: parentNode.id,
                    children: [targetId, newNodeId],
                    size: targetNode.size,
                    pixelSize: targetNode.pixelSize
                };

                const targetIndex = parentNode.children?.indexOf(targetId) || 0;
                parentNode.children![targetIndex] = branchId;

                targetNode.parentId = branchId;
                targetNode.size = 1;
                targetNode.pixelSize = undefined;

                newNode.parentId = branchId;

                state.nodes[newNodeId] = newNode;
                state.nodes[branchId] = branch;
            }
        }),

        resizeNode: (nodeId, size) => set((state) => {
            const node = state.nodes[nodeId];
            if (node) {
                node.pixelSize = size;
            }
        }),

        moveNode: (sourceId, targetId, position) => set((state) => {
            console.log('Moving node', sourceId, 'to', targetId, position);
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

            // 从源节点移除
            sourceNode.data!.tabs = sourceTabs.filter(t => t.id !== tabId);
            if (sourceNode.data!.activeTabId === tabId) {
                sourceNode.data!.activeTabId = sourceNode.data!.tabs[0]?.id;
            }

            // VS Code 逻辑：如果源节点没有 tabs 了，且不是最后一个编辑器组，移除源节点
            if (sourceNode.data!.tabs.length === 0) {
                const editorGroups = Object.values(state.nodes).filter(n => n.type === 'component' && n.data?.tabs);
                if (editorGroups.length > 1) {
                    const parent = state.nodes[sourceNode.parentId!];
                    if (parent && parent.children) {
                        parent.children = parent.children.filter(id => id !== sourceNodeId);
                    }
                    delete state.nodes[sourceNodeId];
                }
            }

            // 添加到目标节点
            const targetTabs = targetNode.data?.tabs || [];
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
        }),

        addTab: (nodeId, tab) => set((state) => {
            const node = state.nodes[nodeId];
            if (!node || node.type !== 'component') return;

            const tabs = node.data?.tabs || [];
            // 如果标签已存在，则激活它
            if (tabs.find(t => t.id === tab.id)) {
                node.data!.activeTabId = tab.id;
                return;
            }

            // 添加新标签
            node.data = {
                ...node.data,
                tabs: [...tabs, tab],
                activeTabId: tab.id,
                component: 'GraphEditor' // 确保是编辑器组
            };
        }),

        setDragging: (isDragging) => set((state) => {
            state.isDragging = isDragging;
        }),

        setActiveGroup: (id) => set((state) => {
            state.activeGroupId = id;

            // 逻辑补充：如果该节点是包含 tabs 的组件，则更新 activeEditorGroupId
            const node = id ? state.nodes[id] : null;
            if (node?.type === 'component' && node.data?.tabs) {
                state.activeEditorGroupId = id;
            }
        }),
    }))
);
