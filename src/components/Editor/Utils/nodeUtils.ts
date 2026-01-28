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

        // Handle variable/data specific initialization
        if ((node.type === 'get_variable' || node.type === 'set_variable' || node.type === 'get_dataframe') &&
            node.variableId && node.variableName) {
            const vType = node.variableType || 'dataframe';
            const isArray = (node as any).variableIsArray || false;
            node.setVariable(node.variableId, node.variableName, vType, isArray);
        }

        if (node.type === 'get_column' && node.initialData) {
            const { columnName, columnType } = node.initialData;
            if (columnName) {
                node.title = `Get ${columnName}`;
                const outputPin = node.outputs.find(p => p.name === 'Column');
                if (outputPin) {
                    outputPin.type = columnType || 'array';
                    outputPin.isArray = true;
                }
            }
        }
    }
    return node;
}
