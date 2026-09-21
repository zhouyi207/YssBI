import { useProjectIOStore } from "@/features/application/project/projectIOStore";
import { useEffect, useState } from "react";
import { getI18n } from "react-i18next";
import {
  pendingUiIntents,
  settleUiIntent,
  subscribeUi,
} from "@/services/workbench/presentationService";
import type { UiIntent, UiIntentReceipt } from "@/shared/types/domain/uiPresentation";
import {
  captureProjectLifecycleState,
  isCurrentProjectIdentity,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { useProjectProjection } from "@/features/application/project/projectProjection";
import { openGraphInEditor } from "@/features/application/editor/openGraphInEditor";
import { resolveGraphResourceMeta } from "@/features/application/editor/openGraphResource";
import { revealGraphProblem } from "@/features/application/editor/revealGraphProblem";
import { hydrateGraphProjection } from "@/features/application/graphProjection/graphProjectionLifecycle";
import { currentProjectionLocale } from "@/features/application/graphProjection/projectionLocale";
import { openInspectableResult } from "@/features/application/execution/openInspectableResult";
import { resultRef } from "@/features/application/results";
import { revealWorkbenchView } from "@/modules/workbench/public";
import { logger } from "@/features/application/observability/appLogger";

async function execute(intent: UiIntent, current: () => boolean): Promise<boolean> {
  if (!current()) return false;
  switch (intent.kind) {
    case "showPanel":
      return (await revealWorkbenchView(intent.panel)) !== null && current();
    case "openResult":
      return openInspectableResult(resultRef(intent.source), getI18n().t.bind(getI18n()));
    case "openGraph": {
      const meta = resolveGraphResourceMeta(intent.graphPath);
      if (!meta) return false;
      const panel = await openGraphInEditor(intent.graphPath, meta.name, meta.type);
      if (!panel || !current()) return false;
      if (!(await useProjectIOStore.getState().loadGraph(intent.graphPath)) || !current())
        return false;
      // A Harness edit notification can still be in flight when its focus request arrives.
      if (
        !(await hydrateGraphProjection(intent.graphPath, currentProjectionLocale())) ||
        !current()
      )
        return false;
      return (
        !intent.nodeId ||
        (await revealGraphProblem(
          intent.graphPath,
          { kind: "node", nodeId: intent.nodeId },
          panel.groupId,
        ))
      );
    }
  }
}

export function useUiIntents() {
  const { projectInstanceId } = useProjectProjection();
  const [connection, setConnection] = useState(0);
  useEffect(() => {
    if (!projectInstanceId) return;
    const identity = captureProjectLifecycleState();
    if (identity.projectInstanceId !== projectInstanceId) return;
    let closed = false;
    let queue = Promise.resolve();
    let resync: Promise<void> | null = null;
    const queued = new Set<string>();
    const current = () =>
      !closed && isCurrentProjectIdentity({ projectInstanceId, epoch: identity.epoch });
    const failed = () => {
      if (current()) logger.app.warn("UI intent delivery failed", "UiIntents");
    };
    const accept = (receipt: UiIntentReceipt) => {
      if (
        receipt.status !== "pending" ||
        !current() ||
        queued.has(receipt.id) ||
        queued.size >= 128
      )
        return;
      queued.add(receipt.id);
      queue = queue
        .then(async () => {
          if (!current() || !(await settleUiIntent(projectInstanceId, receipt.id, "claimed")))
            return;
          let applied = false;
          try {
            applied = await execute(receipt.intent, current);
          } finally {
            if (current())
              await settleUiIntent(projectInstanceId, receipt.id, applied ? "applied" : "failed");
          }
        })
        .catch(failed)
        .finally(() => queued.delete(receipt.id));
    };
    const recover = () => {
      if (resync || !current()) return;
      resync = pendingUiIntents(projectInstanceId)
        .then((receipts) => {
          if (current()) receipts.forEach(accept);
        })
        .catch(failed)
        .finally(() => {
          resync = null;
        });
    };
    const subscription = subscribeUi(
      projectInstanceId,
      true,
      (event) => {
        if (event.kind === "sessionChanged") {
          closed = true;
          setConnection((value) => value + 1);
          return;
        }
        if (event.kind === "intent") accept(event.receipt);
        else if (event.kind === "resync") recover();
      },
      () => {
        failed();
        recover();
      },
    )
      .then(async (close) => {
        if (!current()) {
          await close();
          return () => Promise.resolve();
        }
        recover();
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
