import { Pin } from './pin';

/// 节点
export interface Node {
    id: string;
    node_type: string;
    category: string[];
    title: string;
    inputs: Pin[];
    outputs: Pin[];
    ui_style: string;
    description?: string;
}

/// 节点 metadata
export interface NodeMetaData {
    ui_style: string;
    description?: string;
    supports_dynamic_pins: boolean;
}

/// 主要是用来初始化 node register
export interface NodeDefinitionDTO {
    name: string;
    category: string[];
    node_metadata: NodeMetaData;
}

/// 这里 dto 本身就是 node definition
export type NodeDefinition = NodeDefinitionDTO;

// 前后端转换辅助函数
export const NodeConverter = {
    fromDTO(dto: NodeDefinitionDTO): NodeDefinition {
        return dto;
    },

    toDTO(node: NodeDefinition): NodeDefinitionDTO {
        return node;
    },
};
