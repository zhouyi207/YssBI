import { describe, expect, it } from 'vitest';
import { createExecutionStreamDrain } from './executionChannelDrain';

describe('createExecutionStreamDrain', () => {
  it('waitForStreamEnd resolves only after executionComplete is handled', async () => {
    const recording: string[] = [];
    const drain = createExecutionStreamDrain((event) => {
      recording.push(event.event);
    });

    const wait = drain.waitForStreamEnd(1);

    drain.onmessage({ event: 'nodeStart', data: { nodeId: 'n1' } });
    drain.onmessage({ event: 'nodeComplete', data: { nodeId: 'n1' } });
    expect(recording).toEqual(['nodeStart', 'nodeComplete']);

    let settled = false;
    void wait.then(() => {
      settled = true;
    });
    await Promise.resolve();
    expect(settled).toBe(false);

    drain.onmessage({ event: 'executionComplete', data: { hasError: false } });
    await wait;

    expect(recording).toEqual(['nodeStart', 'nodeComplete', 'executionComplete']);
  });

  it('skips wait when no graph was executed', async () => {
    const drain = createExecutionStreamDrain();
    await expect(drain.waitForStreamEnd(0)).resolves.toBeUndefined();
  });
});
