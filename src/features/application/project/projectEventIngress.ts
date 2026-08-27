import type { ErrorReference } from '@/services/ipc';
import type {
  ProjectEvent,
  ProjectEventReconciler,
  ProjectReconciliationOutcome,
} from './projectEventReconciler';

type Awaitable<T> = T | PromiseLike<T>;

export type ProjectEventStreamItem =
  | { readonly kind: 'event'; readonly event: ProjectEvent }
  | { readonly kind: 'failure'; readonly issue: ErrorReference };

export type ProjectEventEnqueueOutcome = 'accepted' | 'closed' | 'overflowRecovery';
export type ProjectEventDrainOutcome = { readonly status: 'drained' };

export type ProjectEventIngressRecoveryReason =
  | 'queueOverflow'
  | 'streamFailure'
  | 'reconcilerRejected'
  | 'recoveryRequested';

export interface ProjectEventIngressIssue {
  readonly code:
    | 'project_event_queue_overflow'
    | 'project_event_stream_failure'
    | 'project_event_reconciliation_rejected'
    | 'project_event_recovery_requested';
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
  const code = reason === 'queueOverflow'
    ? 'project_event_queue_overflow'
    : reason === 'streamFailure'
      ? 'project_event_stream_failure'
      : reason === 'reconcilerRejected'
        ? 'project_event_reconciliation_rejected'
        : 'project_event_recovery_requested';
  return { code, incidentId, reason };
}

export function createProjectEventIngress(
  reconciler: ProjectEventReconciler,
  dependencies: ProjectEventIngressDependencies & {
    readonly capacity?: number;
  },
): ProjectEventIngress {
  const capacity = dependencies.capacity ?? DEFAULT_PROJECT_EVENT_QUEUE_CAPACITY;
  const queue: ProjectEventStreamItem[] = [];
  let state: 'open' | 'recovering' | 'closed' = 'open';
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

  const beginRecovery = (
    reason: ProjectEventIngressRecoveryReason,
    incidentId: string | null,
    waitForActive: Promise<void> | null,
  ): Promise<void> => {
    queue.length = 0;
    if (recovery) return recovery;
    if (state !== 'closed') state = 'recovering';
    publishIssue(issueFor(reason, incidentId));

    let recoveryPromise!: Promise<void>;
    recoveryPromise = waitFor(waitForActive)
      .then(async () => {
        try {
          await dependencies.requestAuthoritativeSnapshot(reason);
        } catch {
          // A failed recovery request still leaves the incremental tail invalid.
        }
      })
      .finally(() => {
        if (recovery === recoveryPromise) recovery = null;
        if (state === 'recovering') state = 'open';
        if (state === 'open' && queue.length > 0) startWorker();
      });
    recovery = recoveryPromise;
    return recoveryPromise;
  };

  const processQueue = async (): Promise<void> => {
    while (state === 'open' && queue.length > 0) {
      const item = queue.shift()!;
      try {
        if (item.kind === 'failure') {
          await beginRecovery('streamFailure', item.issue.incidentId, null);
          return;
        }
        const outcome: ProjectReconciliationOutcome = await reconciler.acceptEvent(item.event);
        if (outcome.status === 'recoveryRequested') {
          await beginRecovery('recoveryRequested', null, null);
          return;
        }
      } catch {
        await beginRecovery('reconcilerRejected', null, null);
        return;
      }
    }
  };

  function startWorker(): void {
    if (active || state !== 'open') return;
    const worker = processQueue();
    let finished!: Promise<void>;
    finished = worker.finally(() => {
      if (active === finished) active = null;
      if (state === 'open' && queue.length > 0) startWorker();
    });
    active = finished;
  }

  const enqueue = (item: ProjectEventStreamItem): ProjectEventEnqueueOutcome => {
    if (state === 'closed') return 'closed';
    if (state === 'recovering') return 'overflowRecovery';
    if (item.kind === 'failure') {
      void beginRecovery('streamFailure', item.issue.incidentId, active);
      return 'overflowRecovery';
    }
    if (queue.length >= capacity) {
      void beginRecovery('queueOverflow', null, active);
      return 'overflowRecovery';
    }
    queue.push(item);
    startWorker();
    return 'accepted';
  };

  const closeAndDrain = (): Promise<ProjectEventDrainOutcome> => {
    if (closedDrain) return closedDrain;
    state = 'closed';
    queue.length = 0;
    closedDrain = Promise.all([waitFor(active), waitFor(recovery)]).then(() => ({
      status: 'drained' as const,
    }));
    return closedDrain;
  };

  return { enqueue, closeAndDrain };
}
