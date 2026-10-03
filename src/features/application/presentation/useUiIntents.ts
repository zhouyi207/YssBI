import { useProjectIOStore } from "@/features/application/project/projectIOStore";
import { useEffect, useState } from "react";
import { subscribeUi } from "@/services/workbench/presentationService";
import type { UiIntent } from "@/shared/types/domain/uiPresentation";
import { createUiIntentDelivery } from "./uiIntentDelivery";
import {
  captureProjectLifecycleState,
  isCurrentProjectIdentity,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { openFileInEditor } from "@/features/application/editor/openFileInEditor";
import { openDatabaseInEditor } from "@/features/application/editor/openDatabaseInEditor";
import { projectPublicationCoordinator } from "@/features/application/editorMutation/projectPublicationCoordinator";
import { resourceKey, useResourceStore } from "@/features/core/resource";
import { revealGraphProblem } from "@/features/application/editor/revealGraphProblem";
import { hydrateGraphProjection } from "@/features/application/graphProjection/graphProjectionLifecycle";
import { currentProjectionLocale } from "@/features/application/graphProjection/projectionLocale";
import { openInspectableResult } from "@/features/application/execution/openInspectableResult";
import { resultRef } from "@/features/application/results";
import { revealWorkbenchView, workbenchLayoutRead } from "@/modules/workbench/public";
import { logger } from "@/utils/frontendLogger";

async function execute(intent: UiIntent, current: () => boolean): Promise<boolean> {
  if (!current()) return false;
  switch (intent.kind) {
    case "showPanel":
      return (await revealWorkbenchView(intent.panel)) !== null && current();
    case "openResult":
      return openInspectableResult(resultRef(intent.source));
    case "openResource": {
      const { resource } = intent;
      const key = resourceKey(resource);
      if (!useResourceStore.getState().resources[key]?.exists) {
        await projectPublicationCoordinator.refreshIndex();
      }
      if (!current() || !useResourceStore.getState().resources[key]?.exists) return false;
      if (resource.kind === "database") await openDatabaseInEditor(resource.id);
      else await openFileInEditor(resource.id, resource.kind);
      const panel = workbenchLayoutRead.getActiveEditorPanel();
      if (
        !current() ||
        panel?.metadata.resourceRef !== resource.id ||
        panel.metadata.resourceKind !== resource.kind
      )
        return false;
      if (resource.kind !== "event_graph" && resource.kind !== "function_graph") return true;
      if (!(await useProjectIOStore.getState().loadGraph(resource.id)) || !current()) return false;
      // A Harness edit notification can still be in flight when its focus request arrives.
      if (!(await hydrateGraphProjection(resource.id, currentProjectionLocale())) || !current())
        return false;
      return (
        !intent.nodeId ||
        (await revealGraphProblem(
          resource.id,
          { kind: "node", nodeId: intent.nodeId },
          panel.groupId,
        ))
      );
    }
  }
}

export function useUiIntents() {
  const projectInstanceId = useProjectIOStore((state) => state.projectInstanceId);
  const [connection, setConnection] = useState(0);
  useEffect(() => {
    if (!projectInstanceId) return;
    const identity = captureProjectLifecycleState();
    if (identity.projectInstanceId !== projectInstanceId) return;
    let closed = false;
    const current = () =>
      !closed && isCurrentProjectIdentity({ projectInstanceId, epoch: identity.epoch });
    const failed = () => {
      if (current()) logger.app.warn("UI intent delivery failed", "UiIntents");
    };
    const delivery = createUiIntentDelivery(
      projectInstanceId,
      current,
      (intent) => execute(intent, current),
      failed,
    );
    const subscription = subscribeUi(
      projectInstanceId,
      true,
      (event) => {
        if (event.kind === "sessionChanged") {
          closed = true;
          setConnection((value) => value + 1);
          return;
        }
        if (event.kind === "intent") void delivery.accept(event.receipt);
        else if (event.kind === "resync") void delivery.recover(true);
      },
      () => {
        failed();
        void delivery.recover(true);
      },
    )
      .then(async (close) => {
        if (!current()) {
          await close();
          return () => Promise.resolve();
        }
        void delivery.recover();
        return close;
      })
      .catch(() => {
        failed();
        return () => Promise.resolve();
      });
    return () => {
      closed = true;
      void subscription.then((close) => close()).catch(failed);
    };
  }, [projectInstanceId, connection]);
}
