import { useCallback, useRef, useEffect } from 'react';
import { NodeService } from '@/services';
import { Node as DomainNode } from '@/shared/types/domain';
import { useLayoutStore, LayoutState } from '@/features/core/layout/layoutStore';
import { executeCommand } from '@/features/core/history';
import { isShellNode } from '@/features/core/dataStore/graphNodeSelectors';
import { logger } from '@/utils/appLogger';

/**
 * Node Management Hook (CQRS Pattern)
 * 
 * 采用命令-事件分离模式管理节点：
 * - 命令流：UI → NodeService → Backend → 执行操作
 * - 事件流：Backend → ProjectListener → NodeEventHandler（直接更新 Store）→ 可选 callbacks（handleNodeCreated 等做 UI 扩展）
 * 
 * 特点：
 * - 后端是唯一的ID生成源
 * - 通过事件系统自动同步状态
 * - 支持多窗口同步
 * - 异步非阻塞操作
 * - 数据直接存储在 ProjectStore 的 graphs[graphId].nodes 中
 */
export function useNodeManagement() {
    // 获取当前活动的 tab ID
    const activeEditorNode = useLayoutStore((s: LayoutState) => 
        s.activeEditorGroupId ? s.nodes[s.activeEditorGroupId] : null
    );
    const activeTabId = activeEditorNode?.data?.activeTabId || null;

    // 待处理操作：用于关联前端请求和后端事件
    // key: 临时标识符, value: 回调函数
    const pendingActionsRef = useRef<Map<string, () => void>>(new Map());

    /**
     * 创建单个节点（CQRS模式）
     * @param nodeType 节点类型
     * @param position 节点位置
     * @param params 实例参数（variableId、subGraphId 等，用于 variable/function/dataframe 节点）
     * @returns Promise<void>
     */
    const createNode = useCallback(
        async (
            nodeType: string,
            position: { x: number; y: number },
            params?: {
                variableId?: string;
                variableName?: string;
                variableType?: string;
                subGraphId?: string;
                dataframeId?: string;
            }
        ): Promise<{ nodeId: string; pinIds: string[] } | undefined> => {
            if (!activeTabId) {
                logger.graph.warn('Cannot create node: no active tab', 'NodeManagement');
                return undefined;
            }

            try {
                const context = await executeCommand(activeTabId, 'CreateNode', {
                    nodeType,
                    x: position.x,
                    y: position.y,
                    params,
                });
                const result = context as { nodeId: string; pinIds: string[] } | undefined;
                return result;
            } catch (error) {
                logger.graph.error(`Failed to create node: ${error instanceof Error ? error.message : String(error)}`, 'NodeManagement');
                throw error;
            }
        },
        [activeTabId]
    );

    /**
     * 批量创建节点（CQRS模式）
     * @param nodeTypes 节点类型数组
     * @param positions 节点位置数组（可选）
     * @returns Promise<string[]> 创建成功的节点ID数组
     */
    const createNodes = useCallback(
        async (
            nodeTypes: string[],
            positions?: Array<{ x: number; y: number }>
        ): Promise<string[]> => {
            if (!activeTabId || nodeTypes.length === 0) {
                logger.graph.warn('Cannot create nodes: no active tab or empty node types', 'NodeManagement');
                return [];
            }

            try {
                // 调用后端批量创建（后端会为每个节点发送NodeCreated事件）
                const nodeIds = await NodeService.batchCreateNodes(
                    activeTabId,
                    nodeTypes.map((nodeType, i) => ({
                        nodeType,
                        x: positions?.[i]?.x,
                        y: positions?.[i]?.y,
                    })),
                );

                return nodeIds;

            } catch (error) {
                logger.graph.error(`Failed to create nodes: ${error instanceof Error ? error.message : String(error)}`, 'NodeManagement');
                return [];
            }
        },
        [activeTabId]
    );

    /**
     * 删除单个节点（CQRS模式）
     * @param nodeId 节点ID
     * @returns Promise<boolean> 是否删除成功
     */
    const deleteNode = useCallback(
        async (nodeId: string): Promise<boolean> => {
            if (!activeTabId) {
                logger.graph.warn('Cannot delete node: no active tab', 'NodeManagement');
                return false;
            }

            if (isShellNode(activeTabId, nodeId)) {
                logger.graph.warn('Skip deleting system-managed shell node', 'NodeManagement');
                return false;
            }

            try {
                // 调用后端命令（后端会发送NodeDeleted事件）
                await NodeService.deleteNode(activeTabId, nodeId);

                return true;

            } catch (error) {
                logger.graph.error(`Failed to delete node: ${error instanceof Error ? error.message : String(error)}`, 'NodeManagement');
                return false;
            }
        },
        [activeTabId]
    );

    /**
     * 批量删除节点（CQRS模式）
     * @param nodeIds 节点ID数组
     * @returns Promise<string[]> 成功删除的节点ID数组
     */
    const deleteNodes = useCallback(
        async (nodeIds: string[]): Promise<string[]> => {
            if (!activeTabId || nodeIds.length === 0) {
                logger.graph.warn('Cannot delete nodes: no active tab or empty node IDs', 'NodeManagement');
                return [];
            }

            // 壳节点（Event Begin / Function Entry/Return）不可删除，静默跳过。
            const deletableIds = nodeIds.filter((id) => !isShellNode(activeTabId, id));
            if (deletableIds.length === 0) return [];

            try {
                // 并行调用后端删除所有节点
                const results = await Promise.allSettled(
                    deletableIds.map(id => NodeService.deleteNode(activeTabId, id))
                );

                // 收集成功删除的节点ID
                const deletedIds: string[] = [];
                results.forEach((result, index) => {
                    if (result.status === 'fulfilled') {
                        deletedIds.push(deletableIds[index]);
                    } else {
                        logger.graph.error(`Failed to delete node: ${deletableIds[index]} - ${String(result.reason)}`, 'NodeManagement');
                    }
                });

                return deletedIds;

            } catch (error) {
                logger.graph.error(`Failed to delete nodes: ${error instanceof Error ? error.message : String(error)}`, 'NodeManagement');
                return [];
            }
        },
        [activeTabId]
    );

    /**
     * 处理 NodeCreated 事件的回调（可选 UI 扩展）
     * Store 已由 NodeEventHandler 更新，此处仅做 UI 相关逻辑（如聚焦、打开属性面板等）
     */
    const handleNodeCreated = useCallback(
        (graphId: string, _nodeId: string, _data: DomainNode) => {
            if (graphId !== activeTabId) return;
            // 可选：聚焦新节点、打开属性面板等
        },
        [activeTabId]
    );

    /**
     * 处理 NodeDeleted 事件的回调（可选 UI 扩展）
     * Store 已由 NodeEventHandler 更新
     */
    const handleNodeDeleted = useCallback(
        (graphId: string, _nodeId: string) => {
            if (graphId !== activeTabId) return;
            // 可选：清除选中状态等
        },
        [activeTabId]
    );

    // 注册事件回调到事件系统
    // 注意：这里需要配合 useProjectSync 使用
    // 实际的事件监听在 useProjectSync 中完成
    useEffect(() => {
        // 清理待处理操作
        return () => {
            pendingActionsRef.current.clear();
        };
    }, [activeTabId]);

    return {
        // 创建操作
        createNode,
        createNodes,

        // 删除操作
        deleteNode,
        deleteNodes,

        // 事件处理器（需要注册到 useProjectSync 的 callbacks 中）
        handleNodeCreated,
        handleNodeDeleted,
    };
}
