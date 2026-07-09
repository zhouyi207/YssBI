import { getNodeDefinitionMeta } from '@/shared/types/domain';
import { useNodeRegistryStore } from '../nodeRegister';

/**
 * 解析单个节点的展示元信息（title / category / uiStyle / description）。
 *
 * 供 `useNodeView` 与 store-native graph selectors 共用，保证展示路径行为一致。
 */
export function resolveNodeViewMeta(n: {
  nodeType?: string;
  title?: string;
  category?: string[];
  uiStyle?: string;
  description?: string;
}): { nodeType: string; title: string; category: string[]; uiStyle: string; description?: string } {
  const nodeType = n.nodeType ?? '';
  const def = useNodeRegistryStore.getState().getDefinition(nodeType);
  const rawTitle = n.title ?? '';
  const useDefName = !rawTitle || rawTitle === nodeType;
  const title = def && useDefName ? def.name : (rawTitle || nodeType);
  const meta = def ? getNodeDefinitionMeta(def) : undefined;
  const uiStyle = n.uiStyle ?? meta?.uiStyle ?? 'default';
  const category = n.category ?? def?.category ?? [];
  const description = n.description ?? meta?.description;
  return { nodeType, title, category, uiStyle, description };
}
