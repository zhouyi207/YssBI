import { PinId } from './id';

/**
 * Domain Types - Node
 * 
 * 这些类型代表核心业务领域模型，与后端数据结构一致
 * 用于：
 * - 后端 API 响应
 * - 数据持久化
 * - 业务逻辑处理
 */

/**
 * 节点实例
 * 代表图中的一个节点实例
 */
export interface NodeData {
    id: string;
    graphId: string;
    node_type: string;
    category: string[];
    title: string;
    inputs: PinId[];
    outputs: PinId[];
    ui_style: string;
    description?: string;
}

/**
 * 节点元数据
 * 描述节点类型的配置信息
 */
export interface NodeMetaData {
    ui_style: string;
    description?: string;
    supports_dynamic_pins: boolean;
}

/**
 * 节点定义 DTO
 * 用于节点注册和初始化
 */
export interface NodeDefinitionDTO {
    name: string;
    category: string[];
    node_metadata: NodeMetaData;
}

/**
 * 节点定义
 * 前端使用的节点定义类型
 */
export type NodeDefinition = NodeDefinitionDTO;

/**
 * 节点位置
 * 节点在画布上的位置信息
 */
export interface NodePosition {
    x: number;
    y: number;
}
