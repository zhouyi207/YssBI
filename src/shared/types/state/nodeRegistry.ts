import type { NodeDefinition } from "../domain/node";
import type { LoadStatus } from "../ui";

/**
 * NodeRegistry 初始化 / 加载状态
 */
export interface NodeRegistryState {
  status: LoadStatus;
  error: string | null;
}

/**
 * NodeDefinition 映射表
 * key   -> node_type
 * value -> NodeDefinition
 */
export type NodeDefinitionMap = Map<string, NodeDefinition>;
