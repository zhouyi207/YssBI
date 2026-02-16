import { Node } from '@/shared/types/ui';
import { Position } from "@/shared/types/ui";
import { NodeService } from "@/services";
import { createNode } from "@/features/core/nodeRegister";

/**
 * @deprecated 请使用 NodeService.createNode(graphId, nodeType, x, y)，由后端分配 ID。
 * 此函数已修正为只传 type 和 position，由后端 create_node 分配 ID。
 */
export async function createNodeInBackend(
    subgraphId: string,
    node: Node
): Promise<Node> {
    const nodeId = await NodeService.createNode(subgraphId, node.node_type, node.position.x, node.position.y);
    (node as any).id = nodeId;
    return node;
}

/**
 * 使用后端 API 删除节点
 * @param subgraphId 子图ID
 * @param nodeId 节点ID
 */
export async function deleteNodeInBackend(
    subgraphId: string,
    nodeId: string
): Promise<void> {
    try {
        console.log(`[deleteNodeInBackend] Deleting node: subgraphId=${subgraphId}, nodeId=${nodeId}`);

        await NodeService.deleteNode(subgraphId, nodeId);

        console.log('[deleteNodeInBackend] Node deleted successfully');
    } catch (error) {
        console.error('[deleteNodeInBackend] Failed to delete node:', error);
        throw error;
    }
}

/**
 * 从模板创建节点（仅用于本地构建请求参数，ID 由后端 create_node 分配）
 */
export function createNodeFromTemplate(
    position: Position,
    _scale: number,
    type: string,
    overrides?: Partial<Node> & { subGraphId?: string }
): Node | null {
    const id = "temp"; // 占位符，实际 ID 由后端 create_node 分配
    const node = createNode(type, id, position);
    if (node && overrides) {
        Object.assign(node, overrides);

        // Handle variable/data specific initialization
        if ((node.node_type === 'get_variable' || node.node_type === 'set_variable' || node.node_type === 'get_dataframe') &&
            node.variableId && node.variableName) {
            const vType = node.variableType || 'dataframe';
            const isArray = (node as any).variableIsArray || false;
            node.setVariable(node.variableId, node.variableName, vType, isArray);
        }

        if (node.node_type === 'get_column' && node.initialData) {
            const { columnName, columnType } = node.initialData;
            if (columnName) {
                node.title = `Get ${columnName}`;
                const outputPin = node.outputs.find((p: any) => p.name === 'Column');
                if (outputPin) {
                    outputPin.node_type = columnType || 'array';
                    outputPin.isArray = true;
                }
            }
        }
    }
    return node;
}
