import { commitEditorPanelPublication } from "@/modules/workbench/public";
import { resourceKey, type ProjectResourceMeta, type ResourceKey } from "@/features/core/resource";

interface DockviewResourceMove {
  readonly from: string;
  readonly to: string;
}

export function commitEditorDockviewPublication(
  moves: Iterable<DockviewResourceMove>,
  authoritativeResources: Readonly<Record<ResourceKey, ProjectResourceMeta>>,
  commitBusinessStores: () => void,
): void | Promise<void> {
  return commitEditorPanelPublication(
    moves,
    (resourceKind, resourceRef) =>
      Boolean(authoritativeResources[resourceKey({ id: resourceRef, kind: resourceKind })]),
    commitBusinessStores,
  );
}
