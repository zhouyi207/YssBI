import {
  createGraphResource as createGraphResourceAction,
  deleteResource,
  duplicateGraphResource,
  renameResource,
  type GraphResourceKind,
} from '@/features/application/resource/resourceActions';

export type { GraphResourceKind };

export async function createGraphResource(
  kind: GraphResourceKind,
  name?: string,
): Promise<string> {
  return createGraphResourceAction(kind, name);
}

export async function renameGraph(
  id: string,
  name: string,
  kind: GraphResourceKind,
): Promise<void> {
  await renameResource({ id, kind }, name);
}

export async function deleteGraph(id: string, kind: GraphResourceKind): Promise<void> {
  await deleteResource({ id, kind });
}

export async function duplicateGraph(id: string): Promise<void> {
  await duplicateGraphResource(id);
}
