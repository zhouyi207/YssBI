/// types —— 只放“结构定义”

import { NodeDefinition } from "@/shared/types/domain";
import { LoadStatus } from "@/shared/types/ui";

/**
 * NodeRegistry 初始化 / 加载状态
 *
 * 语义约定：
 * - Idle    : 尚未开始加载
 * - Loading : 正在从 backend 同步
 * - Ready   : 已成功加载，可安全使用
 * - Error   : 加载失败，error 字段包含错误信息
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
