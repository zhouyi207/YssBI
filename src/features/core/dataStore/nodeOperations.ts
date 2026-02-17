import { Node } from '@/shared/types/ui';
import { Position } from "@/shared/types/ui";
import { NodeService } from "@/services";
import { useNodeRegistryStore } from "@/features/core/nodeRegister";

/**
 * 从类型和位置创建 Node 实例（用于本地构建，ID 由后端分配）
 */
function buildNodeFromType(type: string, id: string, position: Position): Node {
    const def = useNodeRegistryStore.getState().getDefinition(type);
    return new Node({
        id,
        nodeType: type,
        category: def?.category ?? [],
        title: def?.name ?? type,
        inputs: [],
        outputs: [],
        uiStyle: def?.node_metadata?.uiStyle ?? def?.node_metadata?.ui_style ?? 'default',
        description: def?.node_metadata?.description,
        position: { x: position.x, y: position.y },
    });
}

/**
 * @deprecated 请使用 NodeService.createNode(graphId, nodeType, x, y)，由后端分配 ID。
 * 此函数已修正为只传 type 和 position，由后端 create_node 分配 ID。
 */
export async function createNodeInBackend(
    subgraphId: string,
    node: Node
): Promise<Node> {
    const nodeId = await NodeService.createNode(subgraphId, node.nodeType, node.position.x, node.position.y);
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
    const node = buildNodeFromType(type, id, position);
    if (overrides) {
        Object.assign(node, overrides);

        // Handle get_column specific initialization
        const initialData = (overrides as { initialData?: { columnName?: string; columnType?: string } }).initialData;
        if (node.nodeType === 'get_column' && initialData?.columnName) {
            node.title = `Get ${initialData.columnName}`;
            const outputPin = node.outputs.find((p: { name: string }) => p.name === 'Column');
            if (outputPin) {
                (outputPin as { type?: string; node_type?: string; isArray?: boolean }).type = initialData.columnType ?? 'array';
                (outputPin as { isArray?: boolean }).isArray = true;
            }
        }
    }
    return node;
}
