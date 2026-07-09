import { invoke } from "@tauri-apps/api/core";
import { logger } from '@/utils/appLogger';
import {
  toBatchCreateNodeIpcItems,
  spawnParamsToInstanceParams,
  type BatchCreateNodeRequest,
  type NodeSpawnParams,
} from '@/shared/types/dto/batchCreateNode';
import { EMPTY_GRAPH_UNDO_PATCH, type GraphUndoPatch } from '@/shared/types/dto/graphUndoPatch';

export type { GraphUndoPatch, NodeSubgraphDTO, ConnectionRebuildDTO } from '@/shared/types/dto/graphUndoPatch';
export type {
  BatchCreateNodeRequest,
  NodeSpawnParams,
  BatchCreateNodeIpcItem,
  NodeInstanceParamsDTO,
} from '@/shared/types/dto/batchCreateNode';

export interface CreateNodeResult {
    nodeId: string;
    pinIds: string[];
}

export interface BatchCreateWithConnectionsEntry extends BatchCreateNodeRequest {
    x: number;
    y: number;
    pins: Array<{
        pinId: string;
        name: string;
        direction: 'input' | 'output';
        userValue?: unknown;
    }>;
}

export class NodeService {
   // ==================== Nodes 操作 ====================

    static async getNodes(graphPath: string): Promise<unknown[]> {
        return await invoke<unknown[]>("get_nodes", { graphPath });
    }

    static async setNodes(graphPath: string, nodes: unknown[]): Promise<void> {
        await invoke("set_nodes", { graphPath, nodes });
    }

    /**
     * 创建单个节点（后端生成和验证）
     */
    static async createNode(
        graphPath: string, 
        nodeType: string,
        x?: number,
        y?: number,
        params?: NodeSpawnParams,
    ): Promise<CreateNodeResult> {
        logger.graph.debug(`Creating node: graphPath=${graphPath}, nodeType=${nodeType}, x=${x}, y=${y}`, 'NodeService');
        const result = await invoke<CreateNodeResult>("create_node", { 
            graphPath, 
            nodeType: nodeType,
            x: x !== undefined ? x : null,
            y: y !== undefined ? y : null,
            params: spawnParamsToInstanceParams(params),
        });
        logger.graph.info(`Node created successfully, ID: ${result.nodeId}`, 'NodeService');
        return result;
    }

    /**
     * Create a node with specific IDs (for redo — preserves node/pin identity)
     */
    static async createNodeWithId(
        graphPath: string,
        nodeId: string,
        pinIds: string[],
        nodeType: string,
        x?: number,
        y?: number,
        params?: NodeSpawnParams,
    ): Promise<void> {
        await invoke("create_node_with_id", {
            graphPath,
            nodeId,
            pinIds,
            nodeType,
            x: x ?? null,
            y: y ?? null,
            params: spawnParamsToInstanceParams(params),
        });
    }

    /**
     * 批量删除节点（单次 IPC）；返回删除前捕获的 undo patch。
     */
    static async batchDeleteNodes(
        graphPath: string,
        nodeIds: string[],
    ): Promise<GraphUndoPatch> {
        if (nodeIds.length === 0) {
            return EMPTY_GRAPH_UNDO_PATCH;
        }
        return await invoke<GraphUndoPatch>("batch_delete_nodes", { graphPath, nodeIds });
    }

    /**
     * Apply a previously captured undo patch (DeleteNodes undo / DisconnectPin undo / Composite redo).
     */
    static async applyGraphPatch(
        graphPath: string,
        patch: GraphUndoPatch,
    ): Promise<void> {
        await invoke("apply_graph_patch", { graphPath, patch });
    }

    /**
     * 批量创建节点（单次 IPC 调用，后端一次性创建并发出 NodesBatchCreated 事件）
     */
    static async batchCreateNodes(
        graphPath: string,
        requests: BatchCreateNodeRequest[],
    ): Promise<string[]> {
        if (requests.length === 0) return [];
        return await invoke<string[]>("batch_create_nodes", {
            graphPath,
            requests: toBatchCreateNodeIpcItems(requests),
        });
    }

    /**
     * 删除单个节点
     */
    static async deleteNode(graphPath: string, nodeId: string): Promise<void> {
        await invoke("delete_node", { graphPath, nodeId });
    }

    /**
     * 批量更新节点位置（拖拽结束时调用，CQRS 模式）
     */
    static async updateNodePositions(
        graphPath: string,
        updates: Array<{ nodeId: string; x: number; y: number }>
    ): Promise<void> {
        if (updates.length === 0) return;
        await invoke("update_node_positions", { graphPath, updates });
    }

    /**
     * Batch-create nodes with pin remapping and connection restoration.
     */
    static async batchCreateWithConnections(
        graphPath: string,
        entries: BatchCreateWithConnectionsEntry[],
        connections: Array<{ fromPin: string; toPin: string }>,
    ): Promise<{
        nodeIds: string[];
        pinMapping: Record<string, string>;
        undoPatch: GraphUndoPatch;
    }> {
        if (entries.length === 0) {
            return { nodeIds: [], pinMapping: {}, undoPatch: EMPTY_GRAPH_UNDO_PATCH };
        }
        return await invoke("batch_create_with_connections", {
            graphPath,
            entries: entries.map((entry) => ({
                nodeType: entry.nodeType,
                x: entry.x,
                y: entry.y,
                params: spawnParamsToInstanceParams(entry.params),
                pins: entry.pins,
            })),
            connections,
        });
    }
}
