import { observeResultRunEvent } from "@/features/application/results";
import {
  pinPreviewCacheKey,
  useExecutionStore,
  type PinPreviewLease,
} from "@/features/core/execution";
import type { GraphOutputRefDto } from "@/shared/types/domain/executionDemand";
import type { RunEvent, RunOutputChannelEvent } from "@/shared/types/domain/runEvent";

export type GraphRunOutcomeState = {
  outcome: "success" | "cancelled" | "error";
};

export type PinPreviewObservation = {
  executionSessionId: string | null;
  output: GraphOutputRefDto;
  generation: number;
  runId: string | null;
  terminal: "pending" | "completed" | "error" | "cancelled";
  stale: boolean;
  lease: PinPreviewLease;
};

function observePinPreviewEvent(
  graphPath: string,
  event: RunEvent,
  preview: PinPreviewObservation,
): void {
  if (!preview.lease.isCurrent()) return;
  if (event.run.graphPath !== graphPath) {
    preview.stale = true;
    return;
  }

  if (event.kind.type === "runStarted") {
    if (preview.runId) {
      preview.stale = true;
      return;
    }
    preview.executionSessionId = event.run.executionSessionId;
    preview.runId = event.run.runId;
    return;
  }
  if (
    !preview.executionSessionId ||
    !preview.runId ||
    event.run.executionSessionId !== preview.executionSessionId ||
    event.run.runId !== preview.runId
  ) {
    preview.stale = true;
    return;
  }
  if (event.kind.type === "runCompleted") {
    preview.terminal = "completed";
    return;
  }
  if (event.kind.type === "runErrored") {
    preview.terminal = "error";
    return;
  }
  if (event.kind.type === "runCancelled") {
    preview.terminal = "cancelled";
    return;
  }
  if (event.kind.type !== "pinPreviewResultReady") return;
  if (
    event.kind.generation !== preview.generation ||
    event.kind.output.graphPath !== preview.output.graphPath ||
    pinPreviewCacheKey(graphPath, event.kind.output.port) !==
      pinPreviewCacheKey(graphPath, preview.output.port)
  ) {
    preview.stale = true;
    return;
  }

  preview.lease.complete(event.kind.resultId);
}

export function observeGraphRunOutput(graphPath: string, event: RunOutputChannelEvent): void {
  useExecutionStore.getState().recordRunOutput(graphPath, event);
}

export function observeGraphRunEvent(
  graphPath: string,
  event: RunEvent,
  state: GraphRunOutcomeState,
  preview?: PinPreviewObservation,
): void {
  if (event.run.graphPath === graphPath) observeResultRunEvent(event);
  if (preview) {
    observePinPreviewEvent(graphPath, event, preview);
    return;
  }
  if (event.run.graphPath !== graphPath) return;
  if (event.kind.type === "runStarted") {
    useExecutionStore.getState().setActiveRunId(graphPath, event.run.runId);
  }
  if (event.kind.type === "runErrored") {
    const execution = useExecutionStore.getState();
    if (execution.getGraph(graphPath).runId !== event.run.runId) return;
    state.outcome = "error";
    execution.recordRunFailure(graphPath, {
      runId: event.run.runId,
      code: event.kind.code,
      phase: event.kind.phase,
      source: event.kind.source,
      incidentId: null,
    });
  }
  if (event.kind.type === "runCancelled") state.outcome = "cancelled";
}
