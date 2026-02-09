/// helpers —— 非 React 的纯函数

import { Position } from "@/shared/types";
import { BaseNode } from "@/views/EditorView/Types/nodes";
import { useNodeRegistryStore } from "./nodeRegistry.store";
import { LoadStatus } from "@/shared/types/loadStatus";

/**
 * 创建节点实例
 *
 * 约定：
 * - 仅在 NodeRegistry 处于 Ready 状态时才会成功
 * - 若定义不存在或 Registry 未准备好，返回 null
 */
export function createNode(
  type: string,
  id: string,
  position: Position,
): BaseNode | null {
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

  return new BaseNode(id, def, position);
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
