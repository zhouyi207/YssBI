import { BaseNode } from "../Types/nodes";
import { ProjectService } from "../../../services/project/projectService";

/**
 * 使用后端 API 创建节点
 * @param subgraphId 子图ID
 * @param node 节点数据
 * @returns 创建成功的节点
 */
export async function createNodeInBackend(
    subgraphId: string,
    node: BaseNode
): Promise<BaseNode> {
    try {
        console.log(`[BACKEND SYNC] Starting sync for node ${node.id} to subgraph ${subgraphId}`);
        console.log(`[createNodeInBackend] Creating node: subgraphId=${subgraphId}, nodeId=${node.id}, nodeType=${node.type}`);

        // 序列化节点为后端格式
        const serializedNode = {
            id: node.id,
            type: node.type,
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
                type: pin.type,
                direction: pin.direction,
                links: pin.links,
                isArray: pin.isArray
            })),
            outputs: node.outputs.map(pin => ({
                id: pin.id,
                nodeId: pin.nodeId,
                name: pin.name,
                type: pin.type,
                direction: pin.direction,
                links: pin.links,
                isArray: pin.isArray
            }))
        };

        // 调用后端 API
        const result = await ProjectService.createNode(subgraphId, serializedNode);

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

        await ProjectService.deleteNode(subgraphId, nodeId);

        console.log('[deleteNodeInBackend] Node deleted successfully');
    } catch (error) {
        console.error('[deleteNodeInBackend] Failed to delete node:', error);
        throw error;
    }
}
