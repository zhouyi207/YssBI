import { Position } from "@/shared/types/ui";

/**
 * 创建节点请求参数（仅用于发送给后端，不构建完整节点）
 * 节点结构由后端 create_node 创建，通过 NodeCreated 事件同步到前端渲染
 */
export interface CreateNodeRequest {
    nodeType: string;
    position: Position;
    /** 可选参数，如 subGraphId（call_function），需后端支持 */
    overrides?: Record<string, unknown>;
}

/**
 * 构建创建节点请求（仅类型+位置+可选参数，不构建完整节点）
 * 发送给后端后，后端 create_node 创建节点并 emit NodeCreated，前端接收事件同步渲染
 */
export function buildCreateNodeRequest(
    nodeType: string,
    position: Position,
    overrides?: Record<string, unknown>
): CreateNodeRequest {
    return { nodeType, position, overrides };
}
