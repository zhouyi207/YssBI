import { openGraphInEditor } from './openGraphInEditor';
import { resourceKey, useResourceStore } from '@/features/core/resource';

export type GraphResourceKind = 'event' | 'function';

export function resolveGraphResourceMeta(
  path: string,
): { name: string; type: GraphResourceKind } | null {
  const resources = useResourceStore.getState().resources;
  const functionMeta = resources[resourceKey({ id: path, kind: 'function' })];
  if (functionMeta) {
    return { name: functionMeta.name, type: 'function' };
  }
  const eventMeta = resources[resourceKey({ id: path, kind: 'event' })];
  if (eventMeta) {
    return { name: eventMeta.name, type: 'event' };
  }
  return null;
}

export async function openGraphResource(
  path: string,
  kind?: GraphResourceKind,
): Promise<void> {
  const meta = kind
    ? {
        name:
          useResourceStore.getState().resources[resourceKey({ id: path, kind })]?.name ?? path,
        type: kind,
      }
    : resolveGraphResourceMeta(path);
  if (!meta) return;
  await openGraphInEditor(path, meta.name, meta.type);
}
