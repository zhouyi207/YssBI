import { useCallback, useRef, useEffect } from 'react';
import { NodeService } from '@/services';
import { Node as DomainNode } from '@/shared/types/domain';
import { useLayoutStore, LayoutState } from '@/features/core/layout/layoutStore';

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
     * @returns Promise<void>
     */
    const createNode = useCallback(
        async (nodeType: string, position: { x: number; y: number }): Promise<void> => {
            if (!activeTabId) {
                console.warn('[useNodeManagement] Cannot create node: no active tab');
                return;
            }

            try {
                // 1. 生成临时key用于关联（使用时间戳 + 随机数确保唯一性）
                const tempKey = `${nodeType}-${Date.now()}-${Math.random()}`;

                // 2. 注册待处理操作（当后端事件到达时会执行）
                pendingActionsRef.current.set(tempKey, () => {
                    // 这里可以添加额外的UI反馈，如选中新创建的节点
                });

                // 3. 调用后端命令（后端会生成真实ID并发送NodeCreated事件）
                await NodeService.createNode(
                    activeTabId,
                    nodeType,
                    position.x,
                    position.y
                );

                // 4. 清除待处理操作
                pendingActionsRef.current.delete(tempKey);

            } catch (error) {
                console.error('[useNodeManagement] Failed to create node:', error);
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
                console.warn('[useNodeManagement] Cannot create nodes: no active tab or empty node types');
                return [];
            }

            try {
                // 调用后端批量创建（后端会为每个节点发送NodeCreated事件）
                const nodeIds = await NodeService.createNodes(
                    activeTabId,
                    nodeTypes,
                    positions
                );

                return nodeIds;

            } catch (error) {
                console.error('[useNodeManagement] Failed to create nodes:', error);
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
                console.warn('[useNodeManagement] Cannot delete node: no active tab');
                return false;
            }

            try {
                // 调用后端命令（后端会发送NodeDeleted事件）
                await NodeService.deleteNode(activeTabId, nodeId);

                return true;

            } catch (error) {
                console.error('[useNodeManagement] Failed to delete node:', error);
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
                console.warn('[useNodeManagement] Cannot delete nodes: no active tab or empty node IDs');
                return [];
            }

            try {
                // 并行调用后端删除所有节点
                const results = await Promise.allSettled(
                    nodeIds.map(id => NodeService.deleteNode(activeTabId, id))
                );

                // 收集成功删除的节点ID
                const deletedIds: string[] = [];
                results.forEach((result, index) => {
                    if (result.status === 'fulfilled') {
                        deletedIds.push(nodeIds[index]);
                    } else {
                        console.error(
                            `[useNodeManagement] Failed to delete node: ${nodeIds[index]}`,
                            result.reason
                        );
                    }
                });

                return deletedIds;

            } catch (error) {
                console.error('[useNodeManagement] Failed to delete nodes:', error);
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
