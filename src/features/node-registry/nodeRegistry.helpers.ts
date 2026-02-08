/// helpers —— 非 React 的纯函数

import { Position } from "@/shared/types";
import { BaseNode } from "@/views/EditorView/Types/nodes";
import { useNodeRegistryStore } from "./nodeRegistry.store";

export function createNode(
    type: string,
    id: string,
    position: Position,
): BaseNode | null {
    const def = useNodeRegistryStore.getState().getDefinition(type);
    if (!def) {
        console.error(`Node type ${type} not found`);
        return null;
    }
    return new BaseNode(id, def, position);
}

export function getNodeDefinition(type: string) {
    return useNodeRegistryStore.getState().getDefinition(type);
}

export function hasNodeDefinition(type: string): boolean {
    return useNodeRegistryStore.getState().hasDefinition(type);
}
