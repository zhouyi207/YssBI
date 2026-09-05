import { currentProjectionLocale } from "@/features/application/graphProjection/graphProjectionLifecycle";
import {
  prepareGraphProjectionReplacements,
  commitPreparedGraphProjectionReplacements,
} from "@/features/core/dataStore/graphProjectionStore";
import { waitForGraphDraftMutations } from "./graphDraftCoordinator";
import { getGraphDraftDocument, useGraphDraftStore } from "@/features/core/graphDraft";
import {
  captureProjectIdentity,
  isCurrentProjectIdentity,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { GraphDraftService } from "@/services/nodeSystem/graphDraftService";

export async function compileGraphDraft(graphPath: string): Promise<boolean> {
  const projectIdentity = captureProjectIdentity();
  await waitForGraphDraftMutations(graphPath);
  if (!isCurrentProjectIdentity(projectIdentity)) return false;
  const draftState = useGraphDraftStore.getState();
  if (!draftState.beginCompile(graphPath)) return false;
  const request = useGraphDraftStore.getState().sessions[graphPath].compileRequest!;
  const draftDocument = getGraphDraftDocument(graphPath);
  if (!draftDocument) {
    draftState.failCompile(graphPath, request);
    throw new Error(`Graph draft '${graphPath}' is not loaded`);
  }

  try {
    const compileReceipt = await GraphDraftService.compile(
      projectIdentity.projectInstanceId,
      graphPath,
      currentProjectionLocale(),
      draftDocument,
    );
    if (
      !isCurrentProjectIdentity(projectIdentity) ||
      !useGraphDraftStore.getState().isCompileCurrent(graphPath, request)
    )
      return false;
    const prepared = prepareGraphProjectionReplacements([
      { graphPath, projection: compileReceipt.projection },
    ]);
    if (!prepared.prepared) {
      throw new Error("Compiled Graph projection could not be installed");
    }
    commitPreparedGraphProjectionReplacements(prepared.plan);
    useGraphDraftStore.getState().completeCompile(graphPath, compileReceipt, request);
    return compileReceipt.type === "ready";
  } catch (error) {
    if (
      !isCurrentProjectIdentity(projectIdentity) ||
      !useGraphDraftStore.getState().isCompileCurrent(graphPath, request)
    )
      return false;
    useGraphDraftStore.getState().failCompile(graphPath, request);
    throw error;
  }
}
