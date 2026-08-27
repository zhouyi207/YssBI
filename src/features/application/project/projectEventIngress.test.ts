import { describe, expect, it, vi } from 'vitest';
import {
  createProjectEventIngress,
  type ProjectEventStreamItem,
} from './projectEventIngress';
import type {
  ProjectEvent,
  ProjectReconciliationOutcome,
  ProjectEventReconciler,
} from './projectEventReconciler';

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

async function flushQueue(): Promise<void> {
  await new Promise<void>((resolve) => setTimeout(resolve, 0));
}

function event(operationId: string): ProjectEvent {
  return {
    type: 'ProjectSaved',
    payload: {
      result: {
        projectInstanceId: 'project-a',
        operationId,
        publicationRevision: 1,
        affectedResources: [],
        indexInvalidated: false,
        history: { canUndo: false, canRedo: false },
      },
    },
  };
}

function item(operationId: string): ProjectEventStreamItem {
  return { kind: 'event', event: event(operationId) };
}

const applied: ProjectReconciliationOutcome = { status: 'applied' };

describe('project event ingress', () => {
  it('serializes the FIFO and removes queued work before a closed drain completes', async () => {
    const first = deferred<ProjectReconciliationOutcome>();
    const accepted: string[] = [];
    const reconciler: ProjectEventReconciler = {
      acceptEvent: vi.fn((received) => {
        accepted.push(received.payload.result.operationId);
        return received.payload.result.operationId === 'a'
          ? first.promise
          : Promise.resolve(applied);
      }),
      acceptCommittedReceipt: vi.fn(async () => applied),
      acknowledgeOperation: vi.fn(),
      rejectOperation: vi.fn(),
      markUnknownOutcome: vi.fn(async () => applied),
      resetForProject: vi.fn(),
    };
    const ingress = createProjectEventIngress(reconciler, {
      capacity: 4,
      requestAuthoritativeSnapshot: vi.fn(async () => undefined),
    });

    expect(ingress.enqueue(item('a'))).toBe('accepted');
    expect(ingress.enqueue(item('b'))).toBe('accepted');
    await Promise.resolve();
    expect(accepted).toEqual(['a']);

    const draining = ingress.closeAndDrain();
    first.resolve(applied);
    await expect(draining).resolves.toEqual({ status: 'drained' });

    expect(accepted).toEqual(['a']);
    expect(ingress.enqueue(item('c'))).toBe('closed');
  });

  it('drops the incremental tail on overflow and performs one recovery before reopening', async () => {
    const first = deferred<ProjectReconciliationOutcome>();
    const recovery = deferred<void>();
    const accepted: string[] = [];
    const recover = vi.fn(() => recovery.promise);
    const reconciler: ProjectEventReconciler = {
      acceptEvent: vi.fn((received) => {
        accepted.push(received.payload.result.operationId);
        return received.payload.result.operationId === 'a'
          ? first.promise
          : Promise.resolve(applied);
      }),
      acceptCommittedReceipt: vi.fn(async () => applied),
      acknowledgeOperation: vi.fn(),
      rejectOperation: vi.fn(),
      markUnknownOutcome: vi.fn(async () => applied),
      resetForProject: vi.fn(),
    };
    const ingress = createProjectEventIngress(reconciler, {
      capacity: 1,
      requestAuthoritativeSnapshot: recover,
    });

    ingress.enqueue(item('a'));
    await Promise.resolve();
    ingress.enqueue(item('b'));
    expect(ingress.enqueue(item('c'))).toBe('overflowRecovery');
    expect(accepted).toEqual(['a']);

    first.resolve(applied);
    await vi.waitFor(() => expect(recover).toHaveBeenCalledOnce());
    expect(accepted).toEqual(['a']);

    recovery.resolve();
    await flushQueue();
    expect(ingress.enqueue(item('d'))).toBe('accepted');
    await flushQueue();
    expect(accepted).toEqual(['a', 'd']);
    await ingress.closeAndDrain();
  });

  it('turns a reconciler rejection into one safe recovery without applying the tail', async () => {
    const accepted: string[] = [];
    const recover = vi.fn(async () => undefined);
    const reconciler: ProjectEventReconciler = {
      acceptEvent: vi.fn((received) => {
        accepted.push(received.payload.result.operationId);
        return received.payload.result.operationId === 'a'
          ? Promise.reject(new Error('transport failure'))
          : Promise.resolve(applied);
      }),
      acceptCommittedReceipt: vi.fn(async () => applied),
      acknowledgeOperation: vi.fn(),
      rejectOperation: vi.fn(),
      markUnknownOutcome: vi.fn(async () => applied),
      resetForProject: vi.fn(),
    };
    const ingress = createProjectEventIngress(reconciler, {
      requestAuthoritativeSnapshot: recover,
    });

    ingress.enqueue(item('a'));
    ingress.enqueue(item('b'));
    await ingress.closeAndDrain();

    expect(accepted).toEqual(['a']);
    expect(recover).toHaveBeenCalledOnce();
  });
});
