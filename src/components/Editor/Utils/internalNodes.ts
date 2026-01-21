import { BaseNode, PinDefinition, NodeDefinition } from "../Types/nodes";
import { PinDefinition as SubGraphPinDef } from "../Types/canvas";
import { Position } from "../../../types";
export function createInternalNode(
    id: string,
    type: string,
    title: string,
    category: string,
    position: Position,
    inputs: PinDefinition[],
    outputs: PinDefinition[],
    isInternal: boolean = true
): BaseNode {
    const def: NodeDefinition = {
        node_type: type,
        category,
        title,
        inputs,
        outputs,
        ui_style: "default"
    };
    const node = new BaseNode(id, def, position);
    node.isInternal = isInternal;
    return node;
}
export function syncInternalNodePins(node: BaseNode, subGraphPins: SubGraphPinDef[], isInputNode: boolean) {
    // For an input node (like Function Entry or Macro Inputs), the subgraph's inputs become the node's OUTPUT pins
    // For an output node (like Function Return or Macro Outputs), the subgraph's outputs become the node's INPUT pins

    const currentPins = isInputNode ? node.outputs : node.inputs;

    // The first pin is always the fixed exec pin (e.g., "In" for macro_inputs, "Out" for macro_outputs, "Then" for function_entry, "In" for function_return)
    const fixedExecPin = currentPins.length > 0 && currentPins[0].type === 'exec' ? currentPins[0] : null;

    // All other pins (including user-defined exec pins) come from subGraphPins
    const existingUserPins = fixedExecPin ? currentPins.slice(1) : currentPins;

    // Create a map of old pin IDs to preserve connections
    const oldPinMap = new Map<string, typeof existingUserPins[0]>();
    existingUserPins.forEach(p => oldPinMap.set(p.id, p));


    // Create new pins based on subGraphPins (these can include exec-type pins defined by the user)
    const newPins = subGraphPins.map((p) => {
        const newPinId = `${node.id}_${isInputNode ? 'out' : 'in'}_${p.id}`;

        // Try to find existing pin by ID first, then by name+type
        const existingPin = oldPinMap.get(newPinId) ||
            existingUserPins.find(oldPin => oldPin.name === p.name && oldPin.type === p.type);

        return {
            id: newPinId,
            nodeId: node.id,
            name: p.name,
            type: p.type as any,
            direction: (isInputNode ? "output" : "input") as any,
            links: existingPin ? existingPin.links : []
        };
    });

    // Update the node's pins: fixed exec pin first (if exists), then user-defined pins

    if (isInputNode) {
        node.outputs = fixedExecPin ? [fixedExecPin, ...newPins] : newPins;
    } else {
        node.inputs = fixedExecPin ? [fixedExecPin, ...newPins] : newPins;
    }
}