/**
 * 节点展示元信息 — 纯函数，不依赖注册表 store。
 * 注册表查找由 `features/domain/nodeViewMeta` 在 hydrate 边界注入。
 */

import { getNodeDefinitionMeta, type NodeDefinition } from './node';

export interface NodeViewMetaInput {
  nodeType?: string;
  title?: string;
  category?: string[];
  description?: string;
}

export interface NodeViewMeta {
  nodeType: string;
  title: string;
  category: string[];
  uiStyle: string;
  description?: string;
}

/** 从节点定义 + 实例快照字段推导展示元信息（uiStyle 仅来自定义）。 */
export function buildNodeViewMeta(
  definition: NodeDefinition | undefined,
  input: NodeViewMetaInput,
): NodeViewMeta {
  const nodeType = input.nodeType ?? '';
  const rawTitle = input.title ?? '';
  const useDefName = !rawTitle || rawTitle === nodeType;
  const title = definition && useDefName ? definition.name : rawTitle || nodeType;
  const meta = definition ? getNodeDefinitionMeta(definition) : undefined;
  const uiStyle = meta?.uiStyle ?? 'default';
  const category = input.category ?? definition?.category ?? [];
  const description = input.description ?? meta?.description;
  return { nodeType, title, category, uiStyle, description };
}
