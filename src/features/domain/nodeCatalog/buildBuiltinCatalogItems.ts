import type { NodeDefinition } from '@/shared/types/domain';
import {
  isCustomNodeDefinition,
  isShellNodeDefinition,
  nodeDefinitionAllowedInGraphKind,
} from '@/shared/types/domain';
import type { NodeCatalogItem } from './types';
import { RESOURCE_SPAWNED_NODE_TYPES } from './types';

function definitionToCatalogItem(node: NodeDefinition): NodeCatalogItem {
  return {
    nodeType: node.nodeType,
    title: node.name,
    category: node.category ?? [],
  };
}

/**
 * Builtin registry nodes for the sidebar catalog (excludes user/project custom definitions).
 */
export function buildBuiltinCatalogItems(
  definitions: NodeDefinition[],
  graphKind?: 'event' | 'function',
): NodeCatalogItem[] {
  const items = definitions
    .filter((node) => !RESOURCE_SPAWNED_NODE_TYPES.has(node.nodeType))
    .filter((node) => !isShellNodeDefinition(node))
    .filter((node) => !isCustomNodeDefinition(node))
    .filter((node) => nodeDefinitionAllowedInGraphKind(node, graphKind))
    .map(definitionToCatalogItem);

  items.sort((a, b) => a.title.localeCompare(b.title));
  return items;
}
