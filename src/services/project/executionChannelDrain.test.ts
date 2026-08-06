import { describe, expect, it, vi } from 'vitest';
import type { RunEvent, RunEventKind } from '@/shared/types/dto/runEvent';
import { createExecutionStreamDrain } from './executionChannelDrain';

function runEvent(kind: RunEventKind): RunEvent {
  return {
    correlation: {
      projectSessionId: 'project-session-1',
      graphPath: 'events/Main.yssbi-event',
      graphRevision: '7',
      registryFingerprint: '0000000000000000000000000000000000000000000000000000000000000000',
      resourceVersions: {},
      compileId: '9',
      selectionDigest: 'demand-selection-a',
      runId: '41',
      nodeId: null,
      nodeTypeId: null,
      parentCall: null,
    },
    basis: {
      graphRevision: '7',
      registryFingerprint: '0000000000000000000000000000000000000000000000000000000000000000',
      resourceVersions: {},
    },
    kind,
  };
}

describe('createExecutionStreamDrain', () => {
  it('resolves only after the terminal event and delivers all queued run events', async () => {
    const recording: string[] = [];
    const drain = createExecutionStreamDrain((event) => {
      recording.push(event.kind.type);
    });

    const wait = drain.waitForStreamEnd();

    drain.onmessage(runEvent({ type: 'runStarted' }));
    drain.onmessage(runEvent({ type: 'operationStarted', operationIndex: 0, activationId: '1' }));
    expect(recording).toEqual(['runStarted', 'operationStarted']);

    let settled = false;
    void wait.then(() => {
      settled = true;
    });
    await Promise.resolve();
    expect(settled).toBe(false);

    drain.onmessage(runEvent({ type: 'runCompleted' }));
    await wait;

    expect(recording).toEqual(['runStarted', 'operationStarted', 'runCompleted']);
  });

  it.each([
    { type: 'runErrored', code: 'kernelFailed' } as const,
    { type: 'runCancelled' } as const,
  ])('treats $type as terminal', async (terminal) => {
    const drain = createExecutionStreamDrain();
    const wait = drain.waitForStreamEnd();

    drain.onmessage(runEvent(terminal));

    await expect(wait).resolves.toBeUndefined();
  });

  it.each([
    { type: 'runCompleted' } as const,
    { type: 'runErrored', code: 'kernelFailed' } as const,
    { type: 'runCancelled' } as const,
  ])('settles $type transport and rejects the waiter when the consumer throws', async (terminal) => {
    const consumerError = new Error(`consumer failed on ${terminal.type}`);
    const drain = createExecutionStreamDrain(() => {
      throw consumerError;
    });
    const wait = drain.waitForStreamEnd();

    expect(() => drain.onmessage(runEvent(terminal))).not.toThrow();

    await expect(wait).rejects.toBe(consumerError);
  });

  it('rejects malformed channel values before callbacks or terminal observation', async () => {
    const callback = vi.fn();
    const drain = createExecutionStreamDrain(callback);
    const wait = drain.waitForStreamEnd();
    const malformed = { ...runEvent({ type: 'runCompleted' }), extra: true };

    expect(() => (drain.onmessage as (value: unknown) => void)(malformed)).not.toThrow();

    expect(callback).not.toHaveBeenCalled();
    await expect(wait).rejects.toThrow('Invalid run event');
  });

  it('rejects a pending waiter when its channel is disposed', async () => {
    const drain = createExecutionStreamDrain();
    const wait = drain.waitForStreamEnd();

    drain.dispose();

    await expect(wait).rejects.toMatchObject({ code: 'execution_channel_disposed' });
  });
});
