import { useGraphEditingStore } from "@/features/core/graphEditing";
import {
  captureProjectIdentity,
  isCurrentProjectIdentity,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { GraphEditingService } from "@/services/nodeSystem/graphEditingService";
import { installGraphSession } from "./graphEditCoordinator";

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
    return installGraphSession(graphPath, update);
  } catch (error) {
    if (!isCurrent()) return false;
    throw error;
  }
}
