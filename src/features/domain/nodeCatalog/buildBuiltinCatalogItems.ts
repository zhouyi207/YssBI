import type { NodeDefinition } from '@/shared/types/domain';
import type { NodeCatalogItem } from './types';

const SPAWN_FROM_RESOURCE = new Set([
  'Variables:Get Variable',
  'Variables:Set Variable',
  'Functions:Call Function',
]);

/** Builtin registry nodes for the sidebar catalog (excludes resource-spawned nodes). */
export function buildBuiltinCatalogItems(definitions: NodeDefinition[]): NodeCatalogItem[] {
  return definitions
    .filter((node) => !SPAWN_FROM_RESOURCE.has(node.nodeType))
    .map((node) => ({
      nodeType: node.nodeType,
      title: node.name,
      category: node.category ?? [],
    }))
    .sort((a, b) => a.title.localeCompare(b.title));
}
