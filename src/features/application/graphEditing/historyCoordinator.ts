import {
  captureProjectIdentity,
  isCurrentProjectIdentity,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { currentProjectionLocale } from "@/features/application/graphProjection/projectionLocale";
import { GraphEditingService } from "@/services/nodeSystem/graphEditingService";
import { enqueueGraphTask, installGraphSession } from "./graphEditCoordinator";
import { isGraphSaving, useGraphEditingStore } from "@/features/core/graphEditing";

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
  const identity = captureProjectIdentity();
  const session = useGraphEditingStore.getState().sessions[graphPath];
  if (!session) return false;
  if (!(direction === "undo" ? session.canUndo : session.canRedo)) return false;
  const isCurrent = () => {
    const current = useGraphEditingStore.getState().sessions[graphPath];
    return (
      isCurrentProjectIdentity(identity) &&
      current?.sessionId === session.sessionId &&
      current.projectionGeneration === session.projectionGeneration
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
  return installGraphSession(graphPath, update);
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
