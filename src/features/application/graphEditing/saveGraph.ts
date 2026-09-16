import { currentProjectionLocale } from "@/features/application/graphProjection/projectionLocale";
import {
  enqueueGraphTask,
  installGraphSession,
} from "@/features/application/graphEditing/graphEditCoordinator";
import { useGraphEditingStore } from "@/features/core/graphEditing";
import {
  captureProjectIdentity,
  isCurrentProjectIdentity,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { markResourceDirty, useResourceStore } from "@/features/core/resource";
import type { ResourceKind } from "@/features/core/resource";
import { GraphEditingService } from "@/services/nodeSystem/graphEditingService";

export async function saveGraph(
  graphPath: string,
  graphKind: Extract<ResourceKind, "event" | "function">,
): Promise<boolean> {
  const identity = captureProjectIdentity();
  const drafts = useGraphEditingStore.getState();
  if (!drafts.beginSave(graphPath)) return false;
  const sessionId = useGraphEditingStore.getState().sessions[graphPath].sessionId;
  const isCurrentSave = () =>
    isCurrentProjectIdentity(identity) &&
    useGraphEditingStore.getState().sessions[graphPath]?.sessionId === sessionId;

  let completed = false;
  try {
    return await enqueueGraphTask(
      graphPath,
      async () => {
        if (!isCurrentSave()) return false;
        const projectionGeneration =
          useGraphEditingStore.getState().sessions[graphPath].projectionGeneration;
        const version = useGraphEditingStore.getState().sessions[graphPath].version;
        const saved = await GraphEditingService.save(
          identity.projectInstanceId,
          graphPath,
          currentProjectionLocale(),
          crypto.randomUUID(),
          version,
        );
        if (
          !isCurrentSave() ||
          useGraphEditingStore.getState().sessions[graphPath].projectionGeneration !==
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
            },
            "save",
          )
        )
          return false;
        useResourceStore
          .getState()
          .patchResource({ id: graphPath, kind: graphKind }, { revision: saved.resourceRevision });
        markResourceDirty({ id: graphPath, kind: graphKind }, saved.editing.dirty);
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
      useGraphEditingStore.getState().failSave(graphPath);
    }
  }
}
