import { currentProjectionLocale } from "@/features/application/graphProjection/graphProjectionLifecycle";
import { useGraphProjectionStore } from "@/features/core/dataStore/graphProjectionStore";
import { getGraphDraftDocument, useGraphDraftStore } from "@/features/core/graphDraft";
import {
  captureProjectIdentity,
  isCurrentProjectIdentity,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { GraphDraftService } from "@/services/nodeSystem/graphDraftService";

export async function compileGraphDraft(graphPath: string): Promise<boolean> {
  const identity = captureProjectIdentity();
  const drafts = useGraphDraftStore.getState();
  if (!drafts.beginCompile(graphPath)) return false;
  const document = getGraphDraftDocument(graphPath);
  if (!document) {
    drafts.failCompile(graphPath);
    throw new Error(`Graph draft '${graphPath}' is not loaded`);
  }
  const source = JSON.stringify(document);

  try {
    const result = await GraphDraftService.compile(
      identity.projectInstanceId,
      graphPath,
      currentProjectionLocale(),
      document,
    );
    if (!isCurrentProjectIdentity(identity)) return false;
    const current = getGraphDraftDocument(graphPath);
    if (!current || JSON.stringify(current) !== source) {
      useGraphDraftStore.getState().failCompile(graphPath);
      return false;
    }
    const generation =
      useGraphProjectionStore.getState().graphEntities[graphPath]?.requestGeneration ?? 0;
    const applied = useGraphProjectionStore
      .getState()
      .replaceProjection(graphPath, result.projection, generation + 1);
    if (!applied.applied) {
      useGraphDraftStore.getState().failCompile(graphPath);
      throw new Error("Compiled Graph projection could not be installed");
    }
    useGraphDraftStore.getState().completeCompile(graphPath, result);
    return true;
  } catch (error) {
    useGraphDraftStore.getState().failCompile(graphPath);
    throw error;
  }
}
