import type { GraphResourceKind } from '@/shared/types/domain/graphResourcePath';
import { lookupGraphResource } from '@/features/core/resource/resourceSelectors';
import { useResourceStore } from '@/features/core/resource';
import { openGraphInEditor } from './openGraphInEditor';

export function resolveGraphResourceMeta(
  path: string,
): { name: string; type: GraphResourceKind } | null {
  const resources = useResourceStore.getState().resources;
  const functionMeta = lookupGraphResource(resources, path, 'function');
  if (functionMeta?.exists) {
    return { name: functionMeta.name, type: 'function' };
  }
  const eventMeta = lookupGraphResource(resources, path, 'event');
  if (eventMeta?.exists) {
    return { name: eventMeta.name, type: 'event' };
  }
  return null;
}

export async function openGraphResource(
  path: string,
  kind?: GraphResourceKind,
): Promise<void> {
  const meta = kind
    ? (() => {
        const resource = lookupGraphResource(useResourceStore.getState().resources, path, kind);
        if (!resource?.exists) return null;
        return { name: resource.name, type: kind };
      })()
    : resolveGraphResourceMeta(path);
  if (!meta) return;
  await openGraphInEditor(path, meta.name, meta.type);
}
