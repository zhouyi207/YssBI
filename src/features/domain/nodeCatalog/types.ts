import type { Node } from '@/shared/types/domain';
import { CALL_FUNCTION_NODE_TYPE } from '@/features/domain/nodeDefinition';

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

/** Registry nodes spawned from project resources (variables / functions), not the static catalog. */
export const RESOURCE_SPAWNED_NODE_TYPES = new Set([
  'Variables:Get Variable',
  'Variables:Set Variable',
  CALL_FUNCTION_NODE_TYPE,
]);

export function catalogItemKey(item: NodeCatalogItem): string {
  return `${item.nodeType}:${item.overrides?.variableId ?? item.overrides?.subGraphId ?? ''}`;
}
