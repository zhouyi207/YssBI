import { commitEditorPanelPublication } from "@/modules/workbench/public";
import {
  resourceKey,
  shouldRetainResourceEditor,
  type ProjectResourceMeta,
  type ResourceKey,
} from "@/features/core/resource";

interface FlexLayoutResourceMove {
  readonly from: string;
  readonly to: string;
}

export function commitEditorLayoutPublication(
  moves: Iterable<FlexLayoutResourceMove>,
  authoritativeResources: Readonly<Record<ResourceKey, ProjectResourceMeta>>,
  deletedResources: ReadonlySet<ResourceKey>,
  commitBusinessStores: () => void,
  isCurrent?: () => boolean,
): void | Promise<void> {
  return commitEditorPanelPublication(
    moves,
    (resourceKind, resourceRef) => {
      const ref = { id: resourceRef, kind: resourceKind };
      const key = resourceKey(ref);
      if (deletedResources.has(key)) return false;
      return shouldRetainResourceEditor(ref, authoritativeResources);
    },
    commitBusinessStores,
    isCurrent,
  );
}
