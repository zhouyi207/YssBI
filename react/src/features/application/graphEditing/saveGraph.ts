import { currentProjectionLocale } from "@/features/application/graphProjection/projectionLocale";
import {
  enqueueGraphTask,
  installGraphSession,
} from "@/features/application/graphEditing/graphEditCoordinator";
import { useResourceStore } from "@/features/core/resource/resourceStore";
import {
  captureProjectIdentity,
  isCurrentProjectIdentity,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import type { ResourceKind } from "@/features/core/resource";
import { GraphEditingService } from "@/services/nodeSystem/graphEditingService";

export async function saveGraph(
  graphPath: string,
  graphKind: Extract<ResourceKind, "event_graph" | "function_graph">,
): Promise<boolean> {
  const identity = captureProjectIdentity();
  const drafts = useResourceStore.getState();
  const sessionId = drafts.sessions[graphPath]?.sessionId;
  if (sessionId === undefined || !drafts.beginGraphSave(graphPath)) return false;
  const isCurrentSave = () =>
    isCurrentProjectIdentity(identity) &&
    useResourceStore.getState().sessions[graphPath]?.sessionId === sessionId;
  if (!isCurrentSave()) return false;

  let completed = false;
  try {
    return await enqueueGraphTask(
      graphPath,
      async () => {
        if (!isCurrentSave()) return false;
        const projectionGeneration =
          useResourceStore.getState().sessions[graphPath].projectionGeneration;
        const version = useResourceStore.getState().sessions[graphPath].version;
        const saved = await GraphEditingService.save(
          identity.projectInstanceId,
          graphPath,
          currentProjectionLocale(),
          crypto.randomUUID(),
          version,
        );
        if (
          !isCurrentSave() ||
          useResourceStore.getState().sessions[graphPath].projectionGeneration !==
            projectionGeneration
        )
          return false;
        if (saved.projectionReplacement.graphPath !== graphPath) {
          throw new Error("Graph save result targets another graph");
        }

        if (
          !installGraphSession(
            graphPath,
            {
              document: saved.document,
              projection: saved.projectionReplacement.projection,
              editing: saved.editing,
              resultState: saved.resultState,
            },
            { mode: "save", resource: { kind: graphKind, revision: saved.resourceRevision } },
          )
        )
          return false;
        completed = true;
        return true;
      },
      false,
    );
  } catch (error) {
    if (!isCurrentSave()) return false;
    throw error;
  } finally {
    if (!completed && isCurrentSave()) {
      useResourceStore.getState().failGraphSave(graphPath);
    }
  }
}
