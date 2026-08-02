import { Channel } from '@tauri-apps/api/core';
import type { RunEvent } from '@/shared/types/dto/runEvent';
import { trackChannel } from '@/services/devHmrIpc';

export class ExecutionChannelDisposedError extends Error {
  readonly code = 'execution_channel_disposed';

  constructor() {
    super('execution event channel was disposed before a terminal event');
    this.name = 'ExecutionChannelDisposedError';
  }
}

export type ExecutionStreamDrain = {
  onmessage: (msg: RunEvent) => void;
  waitForStreamEnd: () => Promise<void>;
  dispose: () => void;
};

type CallbackError = { caught: unknown };
type StreamSettlement =
  | { reason: 'terminal'; callbackError?: CallbackError }
  | { reason: 'disposed' };

function deliverRunEvent(
  event: RunEvent,
  onEvent: ((event: RunEvent) => void) | undefined,
  settle: (settlement: StreamSettlement) => void,
): void {
  const terminal = event.kind.type === 'runCompleted'
    || event.kind.type === 'runErrored'
    || event.kind.type === 'runCancelled';
  if (!terminal) {
    onEvent?.(event);
    return;
  }

  let callbackError: CallbackError | undefined;
  try {
    onEvent?.(event);
  } catch (caught) {
    callbackError = { caught };
  } finally {
    settle({ reason: 'terminal', callbackError });
  }
}

/** Channel event handler + post-invoke drain (testable without Tauri Channel). */
export function createExecutionStreamDrain(
  onEvent?: (event: RunEvent) => void,
): ExecutionStreamDrain {
  let resolveEnd: ((settlement: StreamSettlement) => void) | undefined;
  let settled = false;
  const streamEnded = new Promise<StreamSettlement>((resolve) => {
    resolveEnd = resolve;
  });
  const settle = (settlement: StreamSettlement) => {
    if (settled) return;
    settled = true;
    resolveEnd?.(settlement);
  };

  return {
    onmessage: (event) => deliverRunEvent(event, onEvent, settle),
    waitForStreamEnd: async () => {
      const settlement = await streamEnded;
      if (settlement.reason === 'disposed') {
        throw new ExecutionChannelDisposedError();
      }
      if (settlement.callbackError) {
        throw settlement.callbackError.caught;
      }
    },
    dispose: () => settle({ reason: 'disposed' }),
  };
}

export type ExecutionChannelBinding = {
  channel: Channel<RunEvent>;
  waitForStreamEnd: () => Promise<void>;
};

/**
 * `invoke("execute_graph_document")` can resolve before the webview drains the
 * Channel queue. Wait for a terminal run event so callers observe every queued event.
 */
export function bindExecutionEventChannel(
  onEvent?: (event: RunEvent) => void,
): ExecutionChannelBinding {
  const drain = createExecutionStreamDrain(onEvent);
  const channel = trackChannel(new Channel<RunEvent>(), drain.dispose);
  channel.onmessage = drain.onmessage;
  return { channel, waitForStreamEnd: drain.waitForStreamEnd };
}
