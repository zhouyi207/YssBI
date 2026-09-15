import { getI18n } from "react-i18next";
import {
  subscribeGraphActivity,
  readExecutionRunState,
} from "@/services/nodeSystem/graphActivityService";
import { useGraphEditingStore } from "@/features/core/graphEditing";
import { useExecutionStore } from "@/features/core/execution";
import {
  isCurrentProjectIdentity,
  type ProjectIdentitySnapshot,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import {
  observeGraphRunEvent,
  type GraphRunOutcomeState,
} from "@/features/application/editor/observeGraphRunEvent";
import { openInspectableResult } from "@/features/application/execution/openInspectableResult";
import { resultRef } from "@/features/application/results";
import { logger } from "@/features/application/observability/appLogger";
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
  const runs = new Map<
    string,
    { state: GraphRunOutcomeState; graphPath: string; executionSessionId: string; runId: string }
  >();
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
        for (const graph of Object.keys(useGraphEditingStore.getState().sessions))
          if (graph !== path) request(graph);
      }
    },
    resync: () => {
      if (!current()) return;
      for (const path of Object.keys(useGraphEditingStore.getState().sessions)) request(path);
      for (const [key, run] of runs)
        void readExecutionRunState(identity.projectInstanceId, run.executionSessionId, run.runId)
          .then((status) => {
            const execution = useExecutionStore.getState();
            if (!current() || execution.getGraph(run.graphPath).runId !== run.runId) return;
            if (status === "succeeded") execution.completeExecution(run.graphPath);
            else if (status === "failed") execution.failExecution(run.graphPath);
            else if (status === "cancelled") execution.interruptExecution(run.graphPath);
            else return;
            runs.delete(key);
          })
          .catch(failed);
    },
    failed,
    execution: (event) => {
      if (!current()) return;
      const path = event.run.graphPath;
      const key = `${event.run.executionSessionId}:${event.run.runId}`;
      const execution = useExecutionStore.getState();
      if (event.kind.type === "runStarted") {
        execution.startExecution(path);
        runs.set(key, {
          state: { outcome: "success" },
          graphPath: path,
          executionSessionId: event.run.executionSessionId,
          runId: event.run.runId,
        });
      }
      const run = runs.get(key)?.state ?? { outcome: "success" as const };
      observeGraphRunEvent(path, event, run);
      if (event.kind.type === "resultInspectionRequested") {
        void openInspectableResult(
          resultRef({
            executionSessionId: event.run.executionSessionId,
            resultId: event.kind.resultId,
          }),
          getI18n().t.bind(getI18n()),
        ).catch(failed);
      }
      if (execution.getGraph(path).runId !== event.run.runId) return;
      switch (event.kind.type) {
        case "runCompleted":
          execution.completeExecution(path);
          runs.delete(key);
          break;
        case "runErrored":
          execution.failExecution(path);
          runs.delete(key);
          break;
        case "runCancelled":
          execution.interruptExecution(path);
          runs.delete(key);
          break;
      }
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
