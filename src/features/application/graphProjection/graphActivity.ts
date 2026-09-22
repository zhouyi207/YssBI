import {
  subscribeGraphActivity,
  readExecutionSnapshot,
} from "@/services/nodeSystem/graphActivityService";
import { useGraphProjectionStore } from "@/features/core/dataStore/graphProjectionStore";
import { useExecutionStore } from "@/features/core/execution";
import {
  isCurrentProjectIdentity,
  type ProjectIdentitySnapshot,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { installGraphRunEvent } from "@/features/application/editor/observeGraphRunEvent";
import { openInspectableResult } from "@/features/application/execution/openInspectableResult";
import { resultRef } from "@/features/application/results";
import { logger } from "@/features/application/observability/appLogger";
import type { RunEvent } from "@/shared/types/domain/runEvent";
import type { GraphEditingStateDto } from "@/shared/types/domain/editorMutation";

type Refresh = (path: string, editing?: GraphEditingStateDto) => Promise<unknown>;
let generation = 0;
let binding: { project: string; ready: Promise<void>; close: (() => Promise<void>) | null } | null =
  null;

export function ensureGraphActivity(
  identity: ProjectIdentitySnapshot,
  refresh: Refresh,
): Promise<void> {
  if (binding?.project === identity.projectInstanceId) return binding.ready;
  resetGraphActivity();
  const epoch = generation;
  const current = () => epoch === generation && isCurrentProjectIdentity(identity);
  const failed = (error: unknown) =>
    logger.graph.error(
      `Graph activity failed: ${error instanceof Error ? error.message : String(error)}`,
      "GraphActivity",
    );
  const request = (path: string, editing?: GraphEditingStateDto) => {
    if (current()) void refresh(path, editing).catch(failed);
  };
  const synchronizationFailed = (error: unknown) => {
    if (!current()) return;
    const execution = useExecutionStore.getState();
    for (const [path, graph] of Object.entries(execution.graphs))
      if (graph.status === "running") execution.markExecutionUnknown(path);
    failed(error);
  };
  let recovery: Promise<void> | null = null;
  let recoverAgain = false;
  const queued: RunEvent[] = [];
  const install = (event: RunEvent) => {
    if (!current()) return;
    if (!installGraphRunEvent(event)) return;
    if (event.kind.type === "resultInspectionRequested") {
      void openInspectableResult(
        resultRef({
          executionSessionId: event.run.executionSessionId,
          resultId: event.kind.resultId,
        }),
      ).catch(failed);
    }
  };
  const recover = () => {
    if (!current()) return;
    if (recovery) {
      recoverAgain = true;
      return;
    }
    recovery = recoverGraphExecution(identity)
      .catch(synchronizationFailed)
      .finally(() => {
        recovery = null;
        if (!current()) {
          queued.length = 0;
          return;
        }
        if (recoverAgain) {
          recoverAgain = false;
          recover();
          return;
        }
        for (const event of queued.splice(0)) install(event);
      });
  };
  const entry = {
    project: identity.projectInstanceId,
    ready: Promise.resolve(),
    close: null as (() => Promise<void>) | null,
  };
  binding = entry;
  entry.ready = subscribeGraphActivity(identity.projectInstanceId, {
    changed: (path, editing) => {
      request(path, editing);
      if (path.startsWith("functions/")) {
        for (const graph of Object.keys(useGraphProjectionStore.getState().sessions))
          if (graph !== path) request(graph);
      }
    },
    resync: () => {
      if (!current()) return;
      for (const path of Object.keys(useGraphProjectionStore.getState().sessions)) request(path);
      recover();
    },
    failed: synchronizationFailed,
    execution: (event) => {
      if (recovery) {
        if (queued.length >= 128) {
          queued.shift();
          recoverAgain = true;
        }
        queued.push(event);
      } else install(event);
    },
  })
    .then(async (subscription) => {
      if (!current()) {
        await subscription.close();
        return;
      }
      entry.close = subscription.close;
    })
    .catch((error: unknown) => {
      if (binding === entry) binding = null;
      throw error;
    });
  return entry.ready;
}

export function resetGraphActivity(): void {
  generation++;
  const previous = binding;
  binding = null;
  if (previous?.close) void previous.close().catch(() => {});
}

export async function recoverGraphExecution(identity: ProjectIdentitySnapshot): Promise<void> {
  const snapshot = await readExecutionSnapshot(identity.projectInstanceId);
  if (!isCurrentProjectIdentity(identity)) return;
  for (const event of snapshot) installGraphRunEvent(event);
}
