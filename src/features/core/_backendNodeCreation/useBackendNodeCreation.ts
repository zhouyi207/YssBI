import { useCallback } from 'react';
import { Node } from '@/shared/types/ui';
import { deleteNodeInBackend } from '@/shared/utils/editor';
import { useEditorGroup } from '@/features/application/editor/core';
import { NodeService } from '@/services';

/**
 * 使用后端创建节点的 Hook
 * 
 * 简化模式：
 * 1. 前端创建节点对象（包含完整信息）
 * 2. 调用后端 create_node 仅用于验证和注册
 * 3. 前端直接添加节点到状态
 */
export function useBackendNodeCreation() {
    const { activeTabId, setNodes, saveHistory } = useEditorGroup();

    /**
     * 创建单个节点
     * @param node 要创建的节点（前端已生成完整信息）
     * @returns Promise<Node | null> 创建成功的节点，失败返回 null
     */
    const createNode = useCallback(
        async (node: Node): Promise<Node | null> => {
            if (!activeTabId) {
                console.warn('[useBackendNodeCreation] No active tab ID, cannot create node');
                return null;
            }

            try {
                console.log(`[useBackendNodeCreation] Creating node via backend: type=${node.node_type || node.type}`);

                // 1. 调用后端创建节点（仅传递类型用于验证）
                await NodeService.createNode(activeTabId, node.node_type || node.type);

                console.log(`[useBackendNodeCreation] Backend validated node successfully`);

                // 2. 添加到前端状态
                setNodes((prev) => [...prev, node]);
                console.log(`[useBackendNodeCreation] Node added to frontend state`);

                // 3. 保存历史记录（在状态更新后）
                if (saveHistory && typeof saveHistory === 'function') {
                    try {
                        saveHistory();
                        console.log(`[useBackendNodeCreation] History saved successfully`);
                    } catch (historyError) {
                        console.error('[useBackendNodeCreation] Failed to save history:', historyError);
                        // 继续执行，不因为历史记录失败而中断
                    }
                } else {
                    console.warn('[useBackendNodeCreation] saveHistory is not available');
                }

                return node;
            } catch (error) {
                console.error('[useBackendNodeCreation] Failed to create node in backend:', error);
                return null;
            }
        },
        [activeTabId, setNodes, saveHistory]
    );

    /**
     * 批量创建节点
     * @param nodes 要创建的节点数组
     * @returns Promise<Node[]> 成功创建的节点数组
     */
    const createNodes = useCallback(
        async (nodes: Node[]): Promise<Node[]> => {
            if (!activeTabId || nodes.length === 0) {
                return [];
            }

            try {
                console.log(`[useBackendNodeCreation] Creating ${nodes.length} nodes via backend...`);

                // 1. 批量调用后端验证
                const nodeTypes = nodes.map(n => n.node_type || n.type);
                await NodeService.createNodes(activeTabId, nodeTypes);

                console.log(`[useBackendNodeCreation] Backend validated all nodes successfully`);

                // 2. 批量添加到前端状态
                setNodes((prev) => [...prev, ...nodes]);

                // 3. 保存历史记录（在状态更新后）
                if (saveHistory && typeof saveHistory === 'function') {
                    try {
                        saveHistory();
                    } catch (historyError) {
                        console.error('[useBackendNodeCreation] Failed to save history:', historyError);
                    }
                }

                return nodes;
            } catch (error) {
                console.error('[useBackendNodeCreation] Failed to create nodes in backend:', error);
                return [];
            }
        },
        [activeTabId, setNodes, saveHistory]
    );

    /**
     * 删除单个节点（后端优先模式）
     * @param nodeId 节点ID
     */
    const deleteNode = useCallback(
        async (nodeId: string): Promise<boolean> => {
            if (!activeTabId) return false;

            try {
                console.log(`[useBackendNodeCreation] Deleting node ${nodeId} from backend first...`);

                // 1. 先调用后端删除节点
                await deleteNodeInBackend(activeTabId, nodeId);

                console.log(`[useBackendNodeCreation] Backend deleted node successfully: nodeId=${nodeId}`);

                // 2. 保存历史记录
                saveHistory();

                // 3. 后端成功后，更新前端状态
                setNodes((prev) => prev.filter((n) => n.id !== nodeId));

                return true;
            } catch (error) {
                console.error('[useBackendNodeCreation] Failed to delete node from backend:', error);
                return false;
            }
        },
        [activeTabId, setNodes, saveHistory]
    );

    /**
     * 批量删除节点（后端优先模式）
     * @param nodeIds 节点ID数组
     * @returns Promise<string[]> 成功删除的节点ID数组
     */
    const deleteNodes = useCallback(
        async (nodeIds: string[]): Promise<string[]> => {
            if (!activeTabId || nodeIds.length === 0) return [];

            try {
                console.log(`[useBackendNodeCreation] Deleting ${nodeIds.length} nodes from backend first...`);

                // 1. 并行调用后端删除所有节点
                const results = await Promise.allSettled(
                    nodeIds.map(id => deleteNodeInBackend(activeTabId, id))
                );

                // 2. 收集成功删除的节点ID
                const deletedIds: string[] = [];
                results.forEach((result, index) => {
                    if (result.status === 'fulfilled') {
                        deletedIds.push(nodeIds[index]);
                    } else {
                        console.error(`[useBackendNodeCreation] Failed to delete node: nodeId=${nodeIds[index]}, reason=${result.reason}`);
                    }
                });

                if (deletedIds.length > 0) {
                    // 3. 保存历史记录
                    saveHistory();

                    // 4. 后端成功后，更新前端状态
                    setNodes((prev) => prev.filter((n) => !deletedIds.includes(n.id)));

                    console.log(`[useBackendNodeCreation] Removed ${deletedIds.length} nodes from frontend`);
                }

                return deletedIds;
            } catch (error) {
                console.error('[useBackendNodeCreation] Failed to delete nodes from backend:', error);
                return [];
            }
        },
        [activeTabId, setNodes, saveHistory]
    );

    return {
        createNode,
        createNodes,
        deleteNode,
        deleteNodes
    };
}
