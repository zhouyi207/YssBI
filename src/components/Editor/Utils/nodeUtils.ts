import { BaseNode } from "../Types/nodes";
import { Position } from "../../../types";
import { NODE_REGISTRY } from "../Nodes/registry";

export function createNodeFromTemplate(
    position: Position,
    _scale: number,
    type: string,
    overrides?: Partial<BaseNode>
): BaseNode | null {
    const id = `node_${Date.now()}`;
    const node = NODE_REGISTRY.createNode(type, id, position);
    if (node && overrides) {
        Object.assign(node, overrides);
    }
    return node;
}
