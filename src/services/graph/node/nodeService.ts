import { invoke } from "@tauri-apps/api/core";
import { logger } from '@/utils/appLogger';
import type { GraphUndoPatch } from './graphUndoPatch';

export type { GraphUndoPatch, NodeSubgraphDTO, ConnectionRebuildDTO } from './graphUndoPatch';

export interface CreateNodeResult {
    nodeId: string;
    pinIds: string[];
}

export class NodeService {
   // ==================== Nodes 操作 ====================

    static async getNodes(subgraphId: string): Promise<unknown[]> {
        return await invoke<unknown[]>("get_nodes", { subgraphId });
    }

    static async setNodes(subgraphId: string, nodes: unknown[]): Promise<void> {
        await invoke("set_nodes", { subgraphId, nodes });
    }

    /**
     * 创建单个节点（后端生成和验证）
     */
    static async createNode(
        subgraphId: string, 
        nodeType: string,
        x?: number,
        y?: number,
        params?: {
            variableId?: string;
            subGraphId?: string;
            dataframeId?: string;
        }
    ): Promise<CreateNodeResult> {
        logger.graph.debug(`Creating node: subgraphId=${subgraphId}, nodeType=${nodeType}, x=${x}, y=${y}`, 'NodeService');
        const taggedParams = params ? NodeService.buildTaggedParams(params) : null;
        const result = await invoke<CreateNodeResult>("create_node", { 
            graphId: subgraphId, 
            nodeType: nodeType,
            x: x !== undefined ? x : null,
            y: y !== undefined ? y : null,
            params: taggedParams,
        });
        logger.graph.info(`Node created successfully, ID: ${result.nodeId}`, 'NodeService');
        return result;
    }

    /**
     * Create a node with specific IDs (for redo — preserves node/pin identity)
     */
    static async createNodeWithId(
        graphId: string,
        nodeId: string,
        pinIds: string[],
        nodeType: string,
        x?: number,
        y?: number,
        params?: {
            variableId?: string;
            subGraphId?: string;
            dataframeId?: string;
        }
    ): Promise<void> {
        const taggedParams = params ? NodeService.buildTaggedParams(params) : null;
        await invoke("create_node_with_id", {
            graphId,
            nodeId,
            pinIds,
            nodeType,
            x: x ?? null,
            y: y ?? null,
            params: taggedParams,
        });
    }

    /**
     * 批量删除节点（单次 IPC）；返回删除前捕获的 undo patch。
     */
    static async batchDeleteNodes(
        graphId: string,
        nodeIds: string[],
    ): Promise<GraphUndoPatch> {
        if (nodeIds.length === 0) {
            return { nodes: [], neighborNodes: [], connections: [] };
        }
        return await invoke<GraphUndoPatch>("batch_delete_nodes", { graphId, nodeIds });
    }

    /**
     * Apply a previously captured undo patch (DeleteNodes undo / DisconnectPin undo / Composite redo).
     */
    static async applyGraphPatch(
        graphId: string,
        patch: GraphUndoPatch,
    ): Promise<void> {
        await invoke("apply_graph_patch", { graphId, patch });
    }

    /**
     * 批量创建节点（单次 IPC 调用，后端一次性创建并发出 NodesBatchCreated 事件）
     */
    static async batchCreateNodes(
        graphId: string,
        requests: Array<{
            nodeType: string;
            x?: number;
            y?: number;
            params?: {
                variableId?: string;
                subGraphId?: string;
                dataframeId?: string;
            };
        }>
    ): Promise<string[]> {
        if (requests.length === 0) return [];
        return await invoke<string[]>("batch_create_nodes", {
            graphId,
            requests: requests.map(r => ({
                nodeType: r.nodeType,
                x: r.x ?? null,
                y: r.y ?? null,
                params: r.params ? NodeService.buildTaggedParams(r.params) : null,
            })),
        });
    }

    /**
     * 删除单个节点
     */
    static async deleteNode(graphId: string, nodeId: string): Promise<void> {
        await invoke("delete_node", { graphId, nodeId });
    }

    /**
     * 批量更新节点位置（拖拽结束时调用，CQRS 模式）
     */
    static async updateNodePositions(
        graphId: string,
        updates: Array<{ nodeId: string; x: number; y: number }>
    ): Promise<void> {
        if (updates.length === 0) return;
        await invoke("update_node_positions", { graphId, updates });
    }

    /**
     * Batch-create nodes with pin remapping and connection restoration.
     */
    static async batchCreateWithConnections(
        graphId: string,
        entries: Array<{
            nodeType: string;
            x: number;
            y: number;
            params?: {
                variableId?: string;
                subGraphId?: string;
                dataframeId?: string;
            };
            pins: Array<{
                pinId: string;
                name: string;
                direction: 'input' | 'output';
                userValue?: unknown;
            }>;
        }>,
        connections: Array<{ fromPin: string; toPin: string }>,
    ): Promise<{
        nodeIds: string[];
        pinMapping: Record<string, string>;
        undoPatch: GraphUndoPatch;
    }> {
        if (entries.length === 0) {
            return { nodeIds: [], pinMapping: {}, undoPatch: { nodes: [], neighborNodes: [], connections: [] } };
        }
        return await invoke("batch_create_with_connections", {
            graphId,
            entries: entries.map(e => ({
                nodeType: e.nodeType,
                x: e.x,
                y: e.y,
                params: e.params ? NodeService.buildTaggedParams(e.params) : null,
                pins: e.pins,
            })),
            connections,
        });
    }

    private static buildTaggedParams(params: {
        variableId?: string;
        subGraphId?: string;
        dataframeId?: string;
    }): Record<string, unknown> {
        if (params.variableId) {
            return {
                paramsKind: 'variable',
                variableId: params.variableId,
            };
        }
        if (params.subGraphId) {
            return { paramsKind: 'subGraph', subGraphId: params.subGraphId };
        }
        if (params.dataframeId) {
            return { paramsKind: 'dataFrame', dataframeId: params.dataframeId };
        }
        return { paramsKind: 'none' };
    }
}
