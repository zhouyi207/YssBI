import { commitEditorPanelPublication } from "@/modules/workbench/public";
import { resourceKey, type ProjectResourceMeta, type ResourceKey } from "@/features/core/resource";

interface FlexLayoutResourceMove {
  readonly from: string;
  readonly to: string;
}

export function commitEditorLayoutPublication(
  moves: Iterable<FlexLayoutResourceMove>,
  authoritativeResources: Readonly<Record<ResourceKey, ProjectResourceMeta>>,
  commitBusinessStores: () => void,
  isCurrent?: () => boolean,
): void | Promise<void> {
  return commitEditorPanelPublication(
    moves,
    (resourceKind, resourceRef) =>
      authoritativeResources[resourceKey({ id: resourceRef, kind: resourceKind })]?.exists === true,
    commitBusinessStores,
    isCurrent,
  );
}
