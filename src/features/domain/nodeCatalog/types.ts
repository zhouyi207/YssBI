import type { Node } from '@/shared/types/domain';

/** Catalog entry for builtin or contextual spawn nodes (palette / sidebar). */
export interface NodeCatalogItem {
  nodeType: string;
  title: string;
  category: string[];
  overrides?: Partial<Node> & {
    subGraphPath?: string;
    variableId?: string;
  };
}

export function catalogItemKey(item: NodeCatalogItem): string {
  return `${item.nodeType}:${item.overrides?.variableId ?? item.overrides?.subGraphPath ?? ''}`;
}
