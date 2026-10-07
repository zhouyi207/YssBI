import { useResourceStore } from "@/features/core/resource/resourceStore";
import { getDocumentState } from "@/features/core/resource";
import { getNodeFileKind } from "@/features/core/resource/resourceSelectors";

/** True when graph body is in memory and does not need a backend reload. */
export function isGraphCachedInMemory(graphPath: string): boolean {
  if (!useResourceStore.getState().hasGraph(graphPath)) return false;

  const kind = getNodeFileKind(graphPath);
  if (!kind) return false;

  const doc = getDocumentState({ id: graphPath, kind });
  if (!doc?.loaded || doc.stale || doc.conflict) return false;

  return true;
}
