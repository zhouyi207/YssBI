import { currentProjectionLocale } from "@/features/application/graphProjection/graphProjectionLifecycle";
import { waitForGraphDraftMutations } from "@/features/application/graphDraft/graphDraftCoordinator";
import {
  prepareGraphProjectionReplacements,
  commitPreparedGraphProjectionReplacements,
} from "@/features/core/dataStore/graphProjectionStore";
import { getGraphDraftDocument, useGraphDraftStore } from "@/features/core/graphDraft";
import {
  captureProjectIdentity,
  isCurrentProjectIdentity,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { markResourceDirty, useResourceStore } from "@/features/core/resource";
import { useHistoryStore } from "@/features/core/history";
import type { ResourceKind } from "@/features/core/resource";
import { GraphDraftService } from "@/services/nodeSystem/graphDraftService";

export async function saveGraphDraft(
  graphPath: string,
  graphKind: Extract<ResourceKind, "event" | "function">,
): Promise<boolean> {
  const identity = captureProjectIdentity();
  const drafts = useGraphDraftStore.getState();
  if (!drafts.beginSave(graphPath)) return false;
  const sessionId = useGraphDraftStore.getState().sessions[graphPath].sessionId;
  const isCurrentSave = () =>
    isCurrentProjectIdentity(identity) &&
    useGraphDraftStore.getState().sessions[graphPath]?.sessionId === sessionId;
  useHistoryStore.setState({ pending: true });

  let completed = false;
  try {
    await waitForGraphDraftMutations(graphPath);
    if (!isCurrentSave()) return false;
    const draftGeneration = useGraphDraftStore.getState().sessions[graphPath].draftGeneration;
    const document = getGraphDraftDocument(graphPath);
    if (!document) throw new Error(`Graph draft '${graphPath}' is not loaded`);
    const saved = await GraphDraftService.save(
      identity.projectInstanceId,
      graphPath,
      currentProjectionLocale(),
      crypto.randomUUID(),
      document,
    );
    if (
      !isCurrentSave() ||
      useGraphDraftStore.getState().sessions[graphPath].draftGeneration !== draftGeneration
    )
      return false;
    if (saved.projectionReplacement.graphPath !== graphPath) {
      throw new Error("Graph save result targets another graph");
    }

    const prepared = prepareGraphProjectionReplacements([saved.projectionReplacement]);
    if (!prepared.prepared) throw new Error("Saved Graph projection could not be installed");
    useGraphDraftStore.getState().completeSave(graphPath, saved);
    commitPreparedGraphProjectionReplacements(prepared.plan);
    useResourceStore
      .getState()
      .patchResource({ id: graphPath, kind: graphKind }, { revision: saved.resourceRevision });
    markResourceDirty({ id: graphPath, kind: graphKind }, false);
    useHistoryStore.setState({ canUndo: false, canRedo: false, pending: false });
    completed = true;
    return true;
  } catch (error) {
    if (!isCurrentSave()) return false;
    throw error;
  } finally {
    if (!completed && isCurrentSave()) {
      useGraphDraftStore.getState().failSave(graphPath);
      const draft = useGraphDraftStore.getState().sessions[graphPath];
      useHistoryStore.setState({
        canUndo: Boolean(draft?.undoStack.length),
        canRedo: Boolean(draft?.redoStack.length),
        pending: false,
      });
    }
  }
}
