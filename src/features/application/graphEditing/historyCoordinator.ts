import {
  prepareGraphProjectionReplacements,
  commitPreparedGraphProjectionReplacements,
} from "@/features/core/dataStore/graphProjectionStore";
import {
  captureProjectIdentity,
  isCurrentProjectIdentity,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { currentProjectionLocale } from "@/features/application/graphProjection/projectionLocale";
import { GraphEditingService } from "@/services/nodeSystem/graphEditingService";
import { enqueueGraphTask, publishGraphEditingState } from "./graphEditCoordinator";
import { isGraphModified, isGraphSaving, useGraphEditingStore } from "@/features/core/graphEditing";
import { markResourceDirty } from "@/features/core/resource";
import { getGraphResourceKind } from "@/features/core/resource/resourceSelectors";

export type HistoryDirection = "undo" | "redo";

export interface ExecuteHistoryMutationInput {
  direction: HistoryDirection;
  graphPath: string;
}

export type ExecuteHistoryMutationOutcome =
  | { status: "applied" }
  | { status: "stale" }
  | { status: "saving" };

async function installHistoryProjection(
  graphPath: string,
  direction: HistoryDirection,
): Promise<boolean> {
  if (isGraphSaving(graphPath)) return false;
  const identity = captureProjectIdentity();
  const session = useGraphEditingStore.getState().sessions[graphPath];
  if (!session) return false;
  if (!(direction === "undo" ? session.canUndo : session.canRedo)) return false;
  const isCurrent = () => {
    const current = useGraphEditingStore.getState().sessions[graphPath];
    return (
      isCurrentProjectIdentity(identity) &&
      current?.sessionId === session.sessionId &&
      current.projectionGeneration === session.projectionGeneration &&
      !current.saving
    );
  };
  let update;
  try {
    update = await GraphEditingService.history(
      identity.projectInstanceId,
      graphPath,
      currentProjectionLocale(),
      session.version,
      direction === "redo",
    );
  } catch (error) {
    if (!isCurrent()) return false;
    throw error;
  }
  if (!isCurrent()) return false;
  const prepared = prepareGraphProjectionReplacements([
    { graphPath, projection: update.projection },
  ]);
  if (!prepared.prepared)
    throw new Error(`Graph draft ${direction} projection could not be installed`);
  useGraphEditingStore.getState().applyTransform(graphPath, update);
  publishGraphEditingState(graphPath, update.editing);
  commitPreparedGraphProjectionReplacements(prepared.plan);
  const kind = getGraphResourceKind(graphPath);
  if (kind) markResourceDirty({ id: graphPath, kind }, isGraphModified(graphPath));
  return true;
}

export async function executeHistoryMutation(
  input: ExecuteHistoryMutationInput,
): Promise<ExecuteHistoryMutationOutcome> {
  if (isGraphSaving(input.graphPath)) return { status: "saving" };
  return enqueueGraphTask(
    input.graphPath,
    async () =>
      (await installHistoryProjection(input.graphPath, input.direction))
        ? { status: "applied" as const }
        : { status: "stale" as const },
    { status: "stale" as const },
  );
}

export function undoEditorHistory(graphPath: string): Promise<ExecuteHistoryMutationOutcome> {
  return executeHistoryMutation({ direction: "undo", graphPath });
}

export function redoEditorHistory(graphPath: string): Promise<ExecuteHistoryMutationOutcome> {
  return executeHistoryMutation({ direction: "redo", graphPath });
}

export function resetHistoryCoordinator(): void {
  useGraphEditingStore.getState().clear();
}
