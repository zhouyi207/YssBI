import type { Node } from '@/shared/types/domain';

/** Catalog entry for builtin or contextual spawn nodes (palette / sidebar). */
export interface NodeCatalogItem {
  nodeType: string;
  title: string;
  category: string[];
  overrides?: Partial<Node> & {
    subGraphId?: string;
    variableId?: string;
  };
}

/** @deprecated Use NodeCatalogItem */
export type PaletteItem = NodeCatalogItem;

export function catalogItemKey(item: NodeCatalogItem): string {
  return `${item.nodeType}:${item.overrides?.variableId ?? item.overrides?.subGraphId ?? ''}`;
}
