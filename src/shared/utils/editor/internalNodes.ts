import { Node } from '@/shared/types/ui';
import { Pin } from "@/shared/types/domain";
import { Pin as GraphPinDef } from "@/shared/types/domain";
import { Position } from "@/shared/types";

// 简化的 Pin 定义（用于创建内部节点时）
type SimplePinDef = {
    name: string;
    type: string;
    isArray?: boolean;
};

export function createInternalNode(
    id: string,
    type: string,
    title: string,
    category: string[],
    position: Position,
    inputs: SimplePinDef[],
    outputs: SimplePinDef[],
    isInternal: boolean = true
): Node {
    // 将简化的 pin 定义转换为完整的 Pin
    const fullInputs: Pin[] = inputs.map((p, idx) => ({
        id: `${id}_in_${idx}`,
        nodeId: id,
        name: p.name,
        type: p.type as any,
        direction: 'input' as const,
        links: [],
        isArray: p.isArray
    }));
    
    const fullOutputs: Pin[] = outputs.map((p, idx) => ({
        id: `${id}_out_${idx}`,
        nodeId: id,
        name: p.name,
        type: p.type as any,
        direction: 'output' as const,
        links: [],
        isArray: p.isArray
    }));
    
    // 创建节点对象
    const node: Node = {
        id,
        type,
        node_type: type,
        category,
        title,
        position,
        inputs: fullInputs,
        outputs: fullOutputs,
        ui_style: "default",
        isInternal,
    } as any;
    
    return node;
}

export function syncInternalNodePins(node: Node, subGraphPins: GraphPinDef[], isInputNode: boolean) {
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
            isArray: p.isArray,
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

export function syncGraphInstanceNodes(nodes: any[], subGraphId: string, inputs?: GraphPinDef[], outputs?: GraphPinDef[], name?: string) {
    return nodes.map((n: any) => {
        if (n.subGraphId !== subGraphId) return n;
        // 深拷贝节点对象
        const newNode = JSON.parse(JSON.stringify(n));
        if (name) newNode.title = name;
        const synchronizePins = (newPinDefs: GraphPinDef[], existingPins: any[], direction: 'input' | 'output') => {
            const execPins = existingPins.filter((p: any) => p.type === 'exec');
            const dataPins = existingPins.filter((p: any) => p.type !== 'exec');
            const newDataPins = newPinDefs.map(newDef => {
                const newPinId = `${newNode.id}_${direction === 'input' ? 'in' : 'out'}_${newDef.id}`;
                const existingPin = dataPins.find((p: any) => p.id === newPinId) || dataPins.find((p: any) => p.name === newDef.name && p.type === newDef.type);
                return { id: newPinId, nodeId: newNode.id, name: newDef.name, type: newDef.type as any, isArray: newDef.isArray, direction, links: existingPin ? existingPin.links : [] };
            });
            return [...execPins, ...newDataPins];
        };
        if (inputs) newNode.inputs = synchronizePins(inputs, n.inputs, 'input') as any;
        if (outputs) newNode.outputs = synchronizePins(outputs, n.outputs, 'output') as any;
        return newNode;
    });
}
