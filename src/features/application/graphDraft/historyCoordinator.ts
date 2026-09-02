import { useGraphProjectionStore } from "@/features/core/dataStore/graphProjectionStore";
import {
  isGraphDraftDirty,
  isGraphDraftSaving,
  useGraphDraftStore,
} from "@/features/core/graphDraft";
import { EMPTY_HISTORY_STATE, useHistoryStore } from "@/features/core/history/historyStore";
import { markResourceDirty } from "@/features/core/resource";
import { inferGraphResourceKind } from "@/shared/types/domain/graphResourcePath";
import type { HistoryStatusDto } from "@/shared/types/domain/editorMutation";

export type HistoryDirection = "undo" | "redo";

export interface ExecuteHistoryMutationInput {
  direction: HistoryDirection;
  graphPath: string;
}

export type ExecuteHistoryMutationOutcome =
  | { status: "applied" }
  | { status: "stale" }
  | { status: "saving" };

function publishDraftHistoryStatus(graphPath: string): void {
  const session = useGraphDraftStore.getState().sessions[graphPath];
  useHistoryStore.setState({
    canUndo: Boolean(session?.undoStack.length),
    canRedo: Boolean(session?.redoStack.length),
    pending: session?.saving === true,
  });
}

function installHistoryProjection(graphPath: string, direction: HistoryDirection): boolean {
  if (isGraphDraftSaving(graphPath)) return false;
  const projection = useGraphDraftStore.getState()[direction](graphPath);
  if (!projection) return false;
  const currentGeneration =
    useGraphProjectionStore.getState().graphEntities[graphPath]?.requestGeneration ?? 0;
  const applied = useGraphProjectionStore
    .getState()
    .replaceProjection(graphPath, projection, currentGeneration + 1);
  if (!applied.applied)
    throw new Error(`Graph draft ${direction} projection could not be installed`);
  const kind = inferGraphResourceKind(graphPath);
  if (kind) markResourceDirty({ id: graphPath, kind }, isGraphDraftDirty(graphPath));
  publishDraftHistoryStatus(graphPath);
  return true;
}

/** Retained for non-Graph resource publications; Graph history is draft-owned. */
export function setHistoryStatus(status: HistoryStatusDto): void {
  useHistoryStore.setState({ canUndo: status.canUndo, canRedo: status.canRedo });
}

export function ensureHistoryStatus(): Promise<void> {
  return Promise.resolve();
}

export async function executeHistoryMutation(
  input: ExecuteHistoryMutationInput,
): Promise<ExecuteHistoryMutationOutcome> {
  if (isGraphDraftSaving(input.graphPath)) return { status: "saving" };
  return installHistoryProjection(input.graphPath, input.direction)
    ? { status: "applied" }
    : { status: "stale" };
}

export function undoEditorHistory(graphPath: string): Promise<ExecuteHistoryMutationOutcome> {
  return executeHistoryMutation({ direction: "undo", graphPath });
}

export function redoEditorHistory(graphPath: string): Promise<ExecuteHistoryMutationOutcome> {
  return executeHistoryMutation({ direction: "redo", graphPath });
}

export function resetHistoryCoordinator(): void {
  useHistoryStore.setState(EMPTY_HISTORY_STATE, true);
  useGraphDraftStore.getState().clear();
}
