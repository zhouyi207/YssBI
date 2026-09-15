import {
  prepareGraphProjectionReplacements,
  commitPreparedGraphProjectionReplacements,
} from "@/features/core/dataStore/graphProjectionStore";
import { useGraphEditingStore } from "@/features/core/graphEditing";
import {
  captureProjectIdentity,
  isCurrentProjectIdentity,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { GraphEditingService } from "@/services/nodeSystem/graphEditingService";
import { publishGraphEditingState } from "./graphEditCoordinator";

// The projection lifecycle calls this inside the graph's shared task queue.
export async function refreshCurrentGraphProjection(
  graphPath: string,
  locale: string,
): Promise<boolean> {
  const identity = captureProjectIdentity();
  const session = useGraphEditingStore.getState().sessions[graphPath];
  if (!isCurrentProjectIdentity(identity) || !session || session.saving) return false;
  const isCurrent = () => {
    const current = useGraphEditingStore.getState().sessions[graphPath];
    return (
      isCurrentProjectIdentity(identity) &&
      current?.sessionId === session.sessionId &&
      current.projectionGeneration === session.projectionGeneration &&
      !current.saving
    );
  };
  try {
    const update = await GraphEditingService.resolve(
      identity.projectInstanceId,
      graphPath,
      locale,
      session.version,
    );
    if (!isCurrent()) return false;
    const prepared = prepareGraphProjectionReplacements([
      { graphPath, projection: update.projection },
    ]);
    if (!prepared.prepared) throw new Error("Resolved Graph projection could not be installed");
    useGraphEditingStore.getState().applyTransform(graphPath, update);
    publishGraphEditingState(graphPath, update.editing);
    commitPreparedGraphProjectionReplacements(prepared.plan);
    return true;
  } catch (error) {
    if (!isCurrent()) return false;
    throw error;
  }
}
