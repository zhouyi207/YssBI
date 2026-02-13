import { useCallback } from 'react';
import { BaseNode } from '@/views/EditorView/Types/nodes';
import { deleteNodeInBackend } from '@/views/EditorView/Utils/backendNodeOps';
import { useCanvas } from '@/features/editor';
import { ProjectService } from '../../../services/project/projectService';
import { SubGraphData } from '@/views/EditorView/Types/canvas';
import { serializeSubGraph, deserializeSubGraph } from '@/views/EditorView/Utils/io';

/**
 * 使用后端创建节点的 Hook
 * 
 * 采用"后端优先"模式：
 * 1. 前端发送创建请求到后端 (createNodes)
 * 2. 后端生成 ID 并修复连接关系
 * 3. 前端接收新节点并渲染
 */
export function useBackendNodeCreation() {
    const { activeTabId, setNodes, saveHistory } = useCanvas();

    /**
     * 批量创建节点（后端优先模式，后端生成 ID）
     * @param nodes 要创建的节点数组
     * @returns Promise<BaseNode[]> 成功创建的节点数组（新 ID）
     */
    const createNodes = useCallback(
        async (nodes: BaseNode[]): Promise<BaseNode[]> => {
            if (!activeTabId || nodes.length === 0) {
                return [];
            }

            try {
                console.log(`[useBackendNodeCreation] Creating ${nodes.length} nodes via backend...`);

                // 1. 序列化
                // 构造临时 SubGraphData 用于序列化 (不包含变量，因为我们只关注节点创建)
                const serializedData = serializeSubGraph(activeTabId, "temp", "event", nodes, { x: 0, y: 0, scale: 1 }, {}, [], []);

                // 2. 调用后端 createNodes (会自动处理 ID 生成和连接重映射)
                const newSerializedNodes = await ProjectService.createNodes(activeTabId, serializedData.nodes);

                // 3. 反序列化
                const tempResData: SubGraphData = {
                    id: activeTabId,
                    name: "temp",
                    type: "event", // 这里的类型不重要
                    nodes: newSerializedNodes,
                    canvas: { x: 0, y: 0, scale: 1 },
                    variables: {},
                    inputs: [],
                    outputs: []
                };
                const { nodes: newBaseNodes } = deserializeSubGraph(tempResData);

                // 4. 更新前端状态
                if (newBaseNodes.length > 0) {
                    saveHistory();
                    setNodes((prev) => [...prev, ...newBaseNodes]);
                    console.log(`[useBackendNodeCreation] Added ${newBaseNodes.length} nodes to frontend (IDs updated by backend)`);
                }

                return newBaseNodes;
            } catch (error) {
                console.error('[useBackendNodeCreation] Failed to create nodes in backend:', error);
                return [];
            }
        },
        [activeTabId, setNodes, saveHistory]
    );

    /**
     * 创建单个节点（后端优先模式，后端生成 ID）
     * @param node 要创建的节点
     * @returns Promise<BaseNode | null> 创建成功的节点，失败返回 null
     */
    const createNode = useCallback(
        async (node: BaseNode): Promise<BaseNode | null> => {
            const results = await createNodes([node]);
            return results.length > 0 ? results[0] : null;
        },
        [createNodes]
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
        deleteNodes,
    };
}

