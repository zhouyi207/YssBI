import { toGraphResourceUri, type GraphResourceKind } from '@/shared/types/domain/graphResourcePath';
import type { ProjectResourceMeta, ResourceKey } from './resourceTypes';

export function lookupGraphResource(
  resources: Readonly<Record<ResourceKey, ProjectResourceMeta>>,
  graphPath: string,
): ProjectResourceMeta | undefined {
  return resources[toGraphResourceUri('event', graphPath)]
    ?? resources[toGraphResourceUri('function', graphPath)];
}

export function lookupGraphResourceByKind(
  resources: Readonly<Record<ResourceKey, ProjectResourceMeta>>,
  graphPath: string,
  kind: GraphResourceKind,
): ProjectResourceMeta | undefined {
  return resources[toGraphResourceUri(kind, graphPath)];
}
