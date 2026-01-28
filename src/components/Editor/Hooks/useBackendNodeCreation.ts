import { useCallback } from 'react';
import { BaseNode } from '../Types/nodes';
import { createNodeInBackend, deleteNodeInBackend } from '../Utils/backendNodeOps';
import { useCanvas } from '../Context/CanvasContext';

/**
 * 使用后端创建节点的 Hook
 */
export function useBackendNodeCreation() {
    const { activeTabId, setNodes, saveHistory } = useCanvas();

    /**
     * 创建节点并同步到后端
     * @param node 要创建的节点
     * @returns Promise<void>
     */
    const createNode = useCallback(
        async (node: BaseNode): Promise<void> => {
            if (!activeTabId) {
                console.warn('[useBackendNodeCreation] No active tab, cannot create node');
                return;
            }

            try {
                // 保存历史记录
                saveHistory();

                // 先在前端添加节点（乐观更新）
                setNodes((prev) => [...prev, node]);

                console.log('[useBackendNodeCreation] Syncing node to backend...', node.id);
                // 后台同步到后端（不阻塞UI）
                createNodeInBackend(activeTabId, node).then(() => {
                    console.log('[useBackendNodeCreation] Backend sync successful for node:', node.id);
                }).catch((error) => {
                    console.error('[useBackendNodeCreation] Failed to sync node to backend:', error);
                    // 后端失败时，可以选择回滚前端状态或显示错误提示
                    // 这里我们保留前端状态，因为已经保存了历史记录，用户可以撤销
                });

            } catch (error) {
                console.error('[useBackendNodeCreation] Failed to create node:', error);
                throw error;
            }
        },
        [activeTabId, setNodes, saveHistory]
    );

    /**
     * 批量创建节点并同步到后端
     * @param nodes 要创建的节点数组
     */
    const createNodes = useCallback(
        async (nodes: BaseNode[]): Promise<void> => {
            if (!activeTabId || nodes.length === 0) {
                return;
            }

            try {
                // 保存历史记录
                saveHistory();

                // 先在前端添加所有节点（乐观更新）
                setNodes((prev) => [...prev, ...nodes]);

                // 后台同步到后端（不阻塞UI）
                Promise.all(
                    nodes.map((node) => createNodeInBackend(activeTabId, node))
                ).catch((error) => {
                    console.error('[useBackendNodeCreation] Failed to sync nodes to backend:', error);
                });
            } catch (error) {
                console.error('[useBackendNodeCreation] Failed to create nodes:', error);
                throw error;
            }
        },
        [activeTabId, setNodes, saveHistory]
    );

    /**
     * 删除单个节点并同步到后端
     * @param nodeId 节点ID
     */
    const deleteNode = useCallback(
        async (nodeId: string): Promise<void> => {
            if (!activeTabId) return;

            try {
                // 保存历史记录
                saveHistory();

                // 前端更新
                setNodes((prev) => prev.filter((n) => n.id !== nodeId));

                console.log(`[BACKEND SYNC] Deleting node ${nodeId} from backend...`);
                // 后台同步
                deleteNodeInBackend(activeTabId, nodeId).catch((error) => {
                    console.error('[useBackendNodeCreation] Failed to sync deletion to backend:', error);
                });
            } catch (error) {
                console.error('[useBackendNodeCreation] Failed to delete node:', error);
                throw error;
            }
        },
        [activeTabId, setNodes, saveHistory]
    );

    /**
     * 批量删除节点并同步到后端
     * @param nodeIds 节点ID数组
     */
    const deleteNodes = useCallback(
        async (nodeIds: string[]): Promise<void> => {
            if (!activeTabId || nodeIds.length === 0) return;

            try {
                saveHistory();

                // 前端更新
                setNodes((prev) => prev.filter((n) => !nodeIds.includes(n.id)));

                console.log(`[BACKEND SYNC] Deleting ${nodeIds.length} nodes from backend...`);
                // 后台同步
                Promise.all(nodeIds.map(id => deleteNodeInBackend(activeTabId, id))).catch((error) => {
                    console.error('[useBackendNodeCreation] Failed to sync batch deletion to backend:', error);
                });
            } catch (error) {
                console.error('[useBackendNodeCreation] Failed to delete nodes:', error);
                throw error;
            }
        },
        [activeTabId, setNodes, saveHistory]
    );

    return {
        createNode,
        createNodes,
        deleteNode,
        deleteNodes,
    };
}

