import { useMemo } from 'react';
import { toGraphResourceUri } from '@/shared/types/domain/graphResourcePath';
import { useResourceStore } from './resourceStore';
import type { ProjectResourceMeta, ResourceKey } from './resourceTypes';

export type GraphResourceRecord = Record<string, Pick<ProjectResourceMeta, 'id' | 'name'>>;

export function lookupGraphResource(
  resources: Record<ResourceKey, ProjectResourceMeta>,
  graphPath: string,
  kind?: 'event' | 'function',
): ProjectResourceMeta | null {
  if (kind) {
    return resources[toGraphResourceUri(kind, graphPath)] ?? null;
  }
  return (
    resources[toGraphResourceUri('event', graphPath)] ??
    resources[toGraphResourceUri('function', graphPath)] ??
    null
  );
}

export function lookupGraphResourceKind(
  resources: Record<ResourceKey, ProjectResourceMeta>,
  graphPath: string,
): 'event' | 'function' | undefined {
  if (resources[toGraphResourceUri('event', graphPath)]?.exists) return 'event';
  if (resources[toGraphResourceUri('function', graphPath)]?.exists) return 'function';
  return undefined;
}

export function selectGraphResourcesByKind(
  resources: Record<ResourceKey, ProjectResourceMeta>,
  kind: 'event' | 'function',
): GraphResourceRecord {
  const result: GraphResourceRecord = {};
  for (const resource of Object.values(resources)) {
    if (resource.kind !== kind || !resource.exists) continue;
    result[resource.id] = {
      id: resource.id,
      name: resource.name,
    };
  }
  return result;
}

export function useGraphResourcesByKind(kind: 'event' | 'function'): GraphResourceRecord {
  const resources = useResourceStore((state) => state.resources);
  return useMemo(() => selectGraphResourcesByKind(resources, kind), [resources, kind]);
}
