import {
  prepareGraphProjectionReplacements,
  commitPreparedGraphProjectionReplacements,
} from "@/features/core/dataStore/graphProjectionStore";
import { useGraphDraftStore } from "@/features/core/graphDraft";
import {
  captureProjectIdentity,
  isCurrentProjectIdentity,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { GraphDraftService } from "@/services/nodeSystem/graphDraftService";
import { waitForGraphDraftMutations } from "./graphDraftCoordinator";

export async function resolveCurrentGraphDraft(
  graphPath: string,
  locale: string,
): Promise<boolean> {
  const identity = captureProjectIdentity();
  await waitForGraphDraftMutations(graphPath);
  const session = useGraphDraftStore.getState().sessions[graphPath];
  if (!isCurrentProjectIdentity(identity) || !session || session.saving) return false;
  const isCurrent = () => {
    const current = useGraphDraftStore.getState().sessions[graphPath];
    return (
      isCurrentProjectIdentity(identity) &&
      current?.sessionId === session.sessionId &&
      current.draftGeneration === session.draftGeneration &&
      !current.saving
    );
  };
  try {
    const projection = await GraphDraftService.resolve(
      identity.projectInstanceId,
      graphPath,
      locale,
      structuredClone(session.document),
    );
    if (!isCurrent()) return false;
    const prepared = prepareGraphProjectionReplacements([{ graphPath, projection }]);
    if (!prepared.prepared) throw new Error("Resolved Graph projection could not be installed");
    useGraphDraftStore.getState().replaceResolvedProjection(graphPath, projection);
    commitPreparedGraphProjectionReplacements(prepared.plan);
    return true;
  } catch (error) {
    if (!isCurrent()) return false;
    throw error;
  }
}
