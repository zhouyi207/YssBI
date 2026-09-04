import { currentProjectionLocale } from "@/features/application/graphProjection/graphProjectionLifecycle";
import { waitForGraphDraftMutations } from "@/features/application/graphDraft/graphDraftCoordinator";
import { useGraphProjectionStore } from "@/features/core/dataStore/graphProjectionStore";
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
  useHistoryStore.setState({ pending: true });

  let completed = false;
  try {
    await waitForGraphDraftMutations(graphPath);
    if (!isCurrentProjectIdentity(identity)) return false;
    const document = getGraphDraftDocument(graphPath);
    if (!document) throw new Error(`Graph draft '${graphPath}' is not loaded`);
    const saved = await GraphDraftService.save(
      identity.projectInstanceId,
      graphPath,
      currentProjectionLocale(),
      crypto.randomUUID(),
      document,
    );
    if (!isCurrentProjectIdentity(identity)) return false;
    if (saved.projectionReplacement.graphPath !== graphPath) {
      throw new Error("Graph save result targets another graph");
    }

    useGraphDraftStore.getState().completeSave(graphPath, saved);
    const currentGeneration =
      useGraphProjectionStore.getState().graphEntities[graphPath]?.requestGeneration ?? 0;
    const applied = useGraphProjectionStore
      .getState()
      .replaceProjection(graphPath, saved.projectionReplacement.projection, currentGeneration + 1);
    if (!applied.applied) throw new Error("Saved Graph projection could not be installed");
    useResourceStore
      .getState()
      .patchResource(
        { id: graphPath, kind: graphKind },
        { revision: saved.projectionReplacement.projection.sourceRevision },
      );
    markResourceDirty({ id: graphPath, kind: graphKind }, false);
    useHistoryStore.setState({ canUndo: false, canRedo: false, pending: false });
    completed = true;
    return true;
  } finally {
    if (!completed) {
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
