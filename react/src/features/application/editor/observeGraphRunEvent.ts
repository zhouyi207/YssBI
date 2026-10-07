import { captureProjectLifecycleState } from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import {
  observeResultRunEvent,
  prepareResultExecutionSession,
} from "@/features/application/results/runtime";
import { useExecutionStore } from "@/features/core/execution";
import type { RunEvent } from "@/shared/types/domain/runEvent";
import { isCurrentGraphRun } from "@/features/core/graph/read";

const observedRuns = new Map<
  string,
  { session: string; run: string; terminal: boolean; inspected: Set<string> }
>();
let observedEpoch = -1;

/** Shared acceptance for the public subscription, invocation stream and recovery snapshot. */
export function installGraphRunEvent(event: RunEvent): boolean {
  const epoch = captureProjectLifecycleState().epoch;
  const current = () =>
    epoch === captureProjectLifecycleState().epoch && isCurrentGraphRun(event.run);
  if (epoch !== observedEpoch) {
    observedRuns.clear();
    observedEpoch = epoch;
  }
  const path = event.run.graphPath;
  const previous = observedRuns.get(path);
  const same =
    previous?.session === event.run.executionSessionId && previous.run === event.run.runId;
  if (
    previous?.session === event.run.executionSessionId &&
    BigInt(previous.run) > BigInt(event.run.runId)
  ) {
    if (current()) {
      if (!prepareResultExecutionSession(event.run.executionSessionId)) return false;
      observeResultRunEvent(event);
      if (current()) useExecutionStore.getState().applyRunEvent(event, false);
    }
    return false;
  }
  const store = useExecutionStore.getState();
  if (same && event.kind.type === "resultInspectionRequested") {
    // A retained result is addressed by its reference, independently of the current graph basis.
    if (previous.inspected.has(event.kind.resultId)) return false;
    previous.inspected.add(event.kind.resultId);
    return true;
  }
  if (event.kind.type === "runStarted") {
    if (same) {
      if (current() && !previous.terminal && store.getGraph(path).status === "unknown")
        store.applyRunEvent(event);
      return false;
    }
    observedRuns.set(path, {
      session: event.run.executionSessionId,
      run: event.run.runId,
      terminal: false,
      inspected: new Set(),
    });
    if (!current()) return false;
  } else if (!same || previous.terminal) {
    return false;
  } else if (!current()) {
    previous.terminal = true;
    return false;
  }
  const accepted = observedRuns.get(path)!;
  if (!prepareResultExecutionSession(event.run.executionSessionId)) return false;
  observeResultRunEvent(event);
  if (!current() || observedRuns.get(path) !== accepted) return false;
  if (event.kind.type !== "runStarted") accepted.terminal = true;
  store.applyRunEvent(event);
  return true;
}
