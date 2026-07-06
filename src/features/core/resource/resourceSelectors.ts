import { useMemo } from 'react';
import { useResourceStore } from './resourceStore';
import type { ProjectResourceMeta, ResourceKey } from './resourceTypes';
import { resourceKey } from './resourceTypes';

export type GraphResourceRecord = Record<string, Pick<ProjectResourceMeta, 'id' | 'name'>>;

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

export function selectFirstGraphResource(
  resources: Record<ResourceKey, ProjectResourceMeta>,
  graphOrder: string[],
): ProjectResourceMeta | null {
  for (const graphId of graphOrder) {
    const eventKey = resourceKey({ id: graphId, kind: 'event' });
    const eventResource = resources[eventKey];
    if (eventResource?.exists) return eventResource;

    const functionKey = resourceKey({ id: graphId, kind: 'function' });
    const functionResource = resources[functionKey];
    if (functionResource?.exists) return functionResource;
  }

  for (const resource of Object.values(resources)) {
    if ((resource.kind === 'event' || resource.kind === 'function') && resource.exists) {
      return resource;
    }
  }
  return null;
}

export function useGraphResourcesByKind(kind: 'event' | 'function'): GraphResourceRecord {
  const resources = useResourceStore((state) => state.resources);
  return useMemo(() => selectGraphResourcesByKind(resources, kind), [resources, kind]);
}
