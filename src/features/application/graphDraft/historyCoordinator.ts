import {
  prepareGraphProjectionReplacements,
  commitPreparedGraphProjectionReplacements,
} from "@/features/core/dataStore/graphProjectionStore";
import {
  captureProjectIdentity,
  isCurrentProjectIdentity,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { currentProjectionLocale } from "@/features/application/graphProjection/graphProjectionLifecycle";
import { GraphDraftService } from "@/services/nodeSystem/graphDraftService";
import { enqueueGraphDraftTask } from "./graphDraftCoordinator";
import {
  isGraphDraftDirty,
  isGraphDraftSaving,
  useGraphDraftStore,
} from "@/features/core/graphDraft";
import { markResourceDirty } from "@/features/core/resource";
import { inferGraphResourceKind } from "@/shared/types/domain/graphResourcePath";

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
  if (isGraphDraftSaving(graphPath)) return false;
  const identity = captureProjectIdentity();
  const session = useGraphDraftStore.getState().sessions[graphPath];
  if (!session) return false;
  const stack = direction === "undo" ? session.undoStack : session.redoStack;
  const version = stack[stack.length - 1];
  if (!version) return false;
  const isCurrent = () => {
    const current = useGraphDraftStore.getState().sessions[graphPath];
    return (
      isCurrentProjectIdentity(identity) &&
      current?.sessionId === session.sessionId &&
      current.draftGeneration === session.draftGeneration &&
      !current.saving
    );
  };
  let projection;
  try {
    projection = await GraphDraftService.resolve(
      identity.projectInstanceId,
      graphPath,
      currentProjectionLocale(),
      structuredClone(version.document),
    );
  } catch (error) {
    if (!isCurrent()) return false;
    throw error;
  }
  if (!isCurrent()) return false;
  const prepared = prepareGraphProjectionReplacements([{ graphPath, projection }]);
  if (!prepared.prepared)
    throw new Error(`Graph draft ${direction} projection could not be installed`);
  if (!useGraphDraftStore.getState()[direction](graphPath, projection)) return false;
  commitPreparedGraphProjectionReplacements(prepared.plan);
  const kind = inferGraphResourceKind(graphPath);
  if (kind) markResourceDirty({ id: graphPath, kind }, isGraphDraftDirty(graphPath));
  return true;
}

export async function executeHistoryMutation(
  input: ExecuteHistoryMutationInput,
): Promise<ExecuteHistoryMutationOutcome> {
  if (isGraphDraftSaving(input.graphPath)) return { status: "saving" };
  return enqueueGraphDraftTask(
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
  useGraphDraftStore.getState().clear();
}
