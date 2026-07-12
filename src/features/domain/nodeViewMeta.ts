/**
 * 节点展示元信息 — 注册表查找 + domain 纯函数组合。
 * hydrate / 事件 / 乐观草稿的统一入口。
 */

import { useNodeRegistryStore } from '@/features/core/nodeRegister';
import {
  buildNodeViewMeta,
  type NodeViewMeta,
  type NodeViewMetaInput,
} from '@/shared/types/domain/nodeViewMeta';

export type { NodeViewMeta, NodeViewMetaInput };

export function resolveNodeViewMeta(input: NodeViewMetaInput): NodeViewMeta {
  const def = useNodeRegistryStore.getState().getDefinition(input.nodeType ?? '');
  return buildNodeViewMeta(def, input);
}
