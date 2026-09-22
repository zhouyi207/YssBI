import { captureProjectLifecycleState } from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { observeResultRunEvent } from "@/features/application/results";
import { useExecutionStore } from "@/features/core/execution";
import type { RunEvent } from "@/shared/types/domain/runEvent";

const observedRuns = new Map<
  string,
  { session: string; run: string; terminal: boolean; inspected: Set<string> }
>();
let observedEpoch = -1;

/** Shared acceptance for the public subscription, invocation stream and recovery snapshot. */
export function installGraphRunEvent(event: RunEvent): boolean {
  const epoch = captureProjectLifecycleState().epoch;
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
    observeResultRunEvent(event);
    return false;
  }
  const store = useExecutionStore.getState();
  if (same && event.kind.type === "resultInspectionRequested") {
    if (previous.inspected.has(event.kind.resultId)) return false;
    previous.inspected.add(event.kind.resultId);
    return true;
  }
  if (event.kind.type === "runStarted") {
    if (same) {
      if (!previous.terminal && store.getGraph(path).status === "unknown")
        store.setActiveRunId(path, event.run.runId);
      return false;
    }
    if (
      !["running", "submitting", "unknown"].includes(store.getGraph(path).status) ||
      store.getGraph(path).runId !== null
    )
      store.startExecution(path);
    observedRuns.set(path, {
      session: event.run.executionSessionId,
      run: event.run.runId,
      terminal: false,
      inspected: new Set(),
    });
  } else if (!same || previous.terminal) {
    return false;
  }
  observeResultRunEvent(event);
  if (event.kind.type === "runStarted") store.setActiveRunId(path, event.run.runId);
  switch (event.kind.type) {
    case "runCompleted":
      store.completeExecution(path);
      break;
    case "runErrored":
      store.recordRunFailure(path, {
        runId: event.run.runId,
        code: event.kind.code,
        phase: event.kind.phase,
        source: event.kind.source,
        incidentId: null,
      });
      store.failExecution(path);
      break;
    case "runCancelled":
      store.interruptExecution(path);
      break;
    default:
      return true;
  }
  observedRuns.get(path)!.terminal = true;
  return true;
}
