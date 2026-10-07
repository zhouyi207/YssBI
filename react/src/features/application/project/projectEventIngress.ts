import type { ProjectEvent } from "@/services/project/projectEventParser";
import type { ProjectEventStreamItem } from "@/services/project/projectEventStream";
import type { ProjectEventConsumer, ProjectEventConsumptionOutcome } from "./projectEventConsumer";

type Awaitable<T> = T | PromiseLike<T>;

export type ProjectEventEnqueueOutcome = "accepted" | "closed" | "overflowRecovery";
export type ProjectEventDrainOutcome = { readonly status: "drained" };

export type ProjectEventIngressRecoveryReason =
  | "queueOverflow"
  | "streamFailure"
  | "consumerRejected"
  | "recoveryRequested";

export interface ProjectEventIngressIssue {
  readonly code:
    | "project_event_queue_overflow"
    | "project_event_stream_failure"
    | "project_event_consumption_rejected"
    | "project_event_recovery_requested";
  readonly incidentId: string | null;
  readonly reason: ProjectEventIngressRecoveryReason;
}

export interface ProjectEventIngressDependencies {
  readonly requestAuthoritativeSnapshot: (
    reason: ProjectEventIngressRecoveryReason,
  ) => Awaitable<void>;
  readonly publishIssue?: (issue: ProjectEventIngressIssue) => void;
}

export interface ProjectEventIngress {
  enqueue(item: ProjectEventStreamItem): ProjectEventEnqueueOutcome;
  closeAndDrain(): Promise<ProjectEventDrainOutcome>;
}

export const DEFAULT_PROJECT_EVENT_QUEUE_CAPACITY = 64;

function issueFor(
  reason: ProjectEventIngressRecoveryReason,
  incidentId: string | null,
): ProjectEventIngressIssue {
  const code =
    reason === "queueOverflow"
      ? "project_event_queue_overflow"
      : reason === "streamFailure"
        ? "project_event_stream_failure"
        : reason === "consumerRejected"
          ? "project_event_consumption_rejected"
          : "project_event_recovery_requested";
  return { code, incidentId, reason };
}

export function createProjectEventIngress(
  consumer: ProjectEventConsumer,
  dependencies: ProjectEventIngressDependencies & {
    readonly capacity?: number;
  },
): ProjectEventIngress {
  const capacity = dependencies.capacity ?? DEFAULT_PROJECT_EVENT_QUEUE_CAPACITY;
  const queue: ProjectEvent[] = [];
  let state: "open" | "recovering" | "closed" = "open";
  let active: Promise<void> | null = null;
  let recovery: Promise<void> | null = null;
  let closedDrain: Promise<ProjectEventDrainOutcome> | null = null;

  const publishIssue = (issue: ProjectEventIngressIssue): void => {
    try {
      dependencies.publishIssue?.(issue);
    } catch {
      // Safe issue presentation is advisory and cannot affect queue ownership.
    }
  };

  const waitFor = (promise: Promise<void> | null): Promise<void> => promise ?? Promise.resolve();

  const requestRecovery = (
    reason: ProjectEventIngressRecoveryReason,
    incidentId: string | null,
  ): void => {
    queue.length = 0;
    if (recovery) return;
    if (state !== "closed") state = "recovering";

    let recoveryPromise!: Promise<void>;
    // Recovery follows the event worker. The worker requests it without waiting back on it.
    recoveryPromise = waitFor(active)
      .then(async () => {
        try {
          await dependencies.requestAuthoritativeSnapshot(reason);
        } catch {
          // A failed recovery request still leaves the incremental tail invalid.
        }
      })
      .finally(() => {
        if (recovery === recoveryPromise) recovery = null;
        if (state === "recovering") state = "open";
        if (state === "open" && queue.length > 0) startWorker();
      });
    recovery = recoveryPromise;
    publishIssue(issueFor(reason, incidentId));
  };

  const processQueue = async (): Promise<void> => {
    while (state === "open" && queue.length > 0) {
      const event = queue.shift()!;
      try {
        const outcome: ProjectEventConsumptionOutcome = await consumer.acceptEvent(event);
        if (outcome.status === "recoveryRequested") {
          requestRecovery("recoveryRequested", null);
          return;
        }
      } catch {
        requestRecovery("consumerRejected", null);
        return;
      }
    }
  };

  function startWorker(): void {
    if (active || state !== "open") return;
    const worker = processQueue();
    let finished!: Promise<void>;
    finished = worker.finally(() => {
      if (active === finished) active = null;
      if (state === "open" && queue.length > 0) startWorker();
    });
    active = finished;
  }

  const enqueue = (item: ProjectEventStreamItem): ProjectEventEnqueueOutcome => {
    if (state === "closed") return "closed";
    if (state === "recovering") return "overflowRecovery";
    if (item.kind === "failure") {
      requestRecovery("streamFailure", item.issue.incidentId);
      return "overflowRecovery";
    }
    if (queue.length >= capacity) {
      requestRecovery("queueOverflow", null);
      return "overflowRecovery";
    }
    queue.push(item.event);
    startWorker();
    return "accepted";
  };

  const closeAndDrain = (): Promise<ProjectEventDrainOutcome> => {
    if (closedDrain) return closedDrain;
    state = "closed";
    queue.length = 0;
    closedDrain = (async () => {
      await waitFor(active);
      // The final event can request recovery after close begins.
      await waitFor(recovery);
      return { status: "drained" as const };
    })();
    return closedDrain;
  };

  return { enqueue, closeAndDrain };
}
