import { Node } from '@/shared/types/ui';
import { Position } from "@/shared/types/ui";
import { NodeService } from "@/services";
import { createNode } from "@/features/core/nodeRegister";

/**
 * 使用后端 API 创建节点
 * @param subgraphId 子图ID
 * @param node 节点数据
 * @returns 创建成功的节点
 */
export async function createNodeInBackend(
    subgraphId: string,
    node: Node
): Promise<Node> {
    try {
        console.log(`[BACKEND SYNC] Starting sync for node ${node.id} to subgraph ${subgraphId}`);
        console.log(`[createNodeInBackend] Creating node: subgraphId=${subgraphId}, nodeId=${node.id}, nodeType=${node.node_type}`);

        // 序列化节点为后端格式
        const serializedNode = {
            id: node.id,
            type: node.node_type,
            title: node.title,
            position: node.position,
            isInternal: node.isInternal,
            variableId: node.variableId,
            variableName: node.variableName,
            variableType: node.variableType,
            subGraphId: node.subGraphId,
            inputs: node.inputs.map(pin => ({
                id: pin.id,
                nodeId: pin.nodeId,
                name: pin.name,
                type: pin.node_type,
                direction: pin.direction,
                links: pin.links,
                isArray: pin.isArray
            })),
            outputs: node.outputs.map(pin => ({
                id: pin.id,
                nodeId: pin.nodeId,
                name: pin.name,
                type: pin.node_type,
                direction: pin.direction,
                links: pin.links,
                isArray: pin.isArray
            }))
        };

        // 调用后端 API
        const result = await NodeService.createNode(subgraphId, serializedNode);

        console.log(`[createNodeInBackend] Node created successfully: ${node.id}, result=${JSON.stringify(result)}`);

        // 返回原始节点对象（后端验证通过）
        return node;
    } catch (error) {
        console.error('[createNodeInBackend] Failed to create node:', error);
        throw error;
    }
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
 * 从模板创建节点
 */
export function createNodeFromTemplate(
    position: Position,
    _scale: number,
    type: string,
    overrides?: Partial<Node> & { subGraphId?: string }
): Node | null {
    const id = `node_${Date.now()}`;
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
