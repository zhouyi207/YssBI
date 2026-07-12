import type { NodeDefinition } from '@/shared/types/domain';
import { isShellNodeDefinition, nodeDefinitionAllowedInGraphKind } from '@/shared/types/domain';
import type { NodeCatalogItem } from './types';
import { RESOURCE_SPAWNED_NODE_TYPES } from './types';

/**
 * Builtin registry nodes for the sidebar catalog.
 * Excludes resource-spawned nodes and system-managed shell nodes (Event Begin, etc.);
 * when `graphKind` is provided, also filters by node graph scope.
 */
export function buildBuiltinCatalogItems(
  definitions: NodeDefinition[],
  graphKind?: 'event' | 'function',
): NodeCatalogItem[] {
  return definitions
    .filter((node) => !RESOURCE_SPAWNED_NODE_TYPES.has(node.nodeType))
    .filter((node) => !isShellNodeDefinition(node))
    .filter((node) => nodeDefinitionAllowedInGraphKind(node, graphKind))
    .map((node) => ({
      nodeType: node.nodeType,
      title: node.name,
      category: node.category ?? [],
    }))
    .sort((a, b) => a.title.localeCompare(b.title));
}
