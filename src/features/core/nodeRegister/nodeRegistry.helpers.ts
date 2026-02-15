/// helpers —— 非 React 的纯函数

import { Position } from "@/shared/types";
import { useNodeRegistryStore } from "./nodeRegistry.store";
import { LoadStatus } from "@/shared/types/ui";

/**
 * 创建节点实例
 *
 * 约定：
 * - 仅在 NodeRegistry 处于 Ready 状态时才会成功
 * - 若定义不存在或 Registry 未准备好，返回 null
 * - type 参数可以是简单名称（如 "Branch"）或完整名称（如 "Control Flow:Branch"）
 */
export function createNode(
  type: string,
  id: string,
  position: Position,
): any | null {
  const { status, getDefinition } = useNodeRegistryStore.getState();

  if (status !== LoadStatus.Ready) {
    console.warn(
      `[createNode] NodeRegistry not ready (status=${status}), cannot create node`,
    );
    return null;
  }

  const def = getDefinition(type);
  if (!def) {
    console.error(`[createNode] Node type "${type}" not found`);
    return null;
  }

  // 构建完整的节点类型名称（category:name 格式）
  const fullNodeType = [...def.category, def.name].join(':');

  // 创建节点对象
  const node: any = {
    id,
    type: fullNodeType,  // 使用完整名称
    node_type: fullNodeType,  // 使用完整名称，后端需要这个格式
    category: def.category || [],
    title: def.name || type,
    position,
    inputs: [],
    outputs: [],
    ui_style: def.node_metadata?.ui_style || "default",
    description: def.node_metadata?.description,
    isInternal: false,
  };

  return node;
}

/**
 * 获取节点定义（只读）
 *
 * - Registry 未 Ready 时返回 undefined
 */
export function getNodeDefinition(type: string) {
  const { status, getDefinition } = useNodeRegistryStore.getState();

  if (status !== LoadStatus.Ready) {
    return undefined;
  }

  return getDefinition(type);
}

/**
 * 判断节点定义是否存在
 *
 * - Registry 未 Ready 时始终返回 false
 */
export function hasNodeDefinition(type: string): boolean {
  const { status, hasDefinition } = useNodeRegistryStore.getState();

  if (status !== LoadStatus.Ready) {
    return false;
  }

  return hasDefinition(type);
}
