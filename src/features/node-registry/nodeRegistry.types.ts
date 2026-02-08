/// types —— 只放“结构定义”

import { NodeDefinition } from "@/views/EditorView/Types/nodes";

/**
 * NodeRegistry 初始化状态
 */
export interface NodeRegistryState {
    isInitialized: boolean;
    isLoading: boolean;
    error: string | null;
}

export type NodeDefinitionMap = Map<string, NodeDefinition>;
