import { Channel } from '@tauri-apps/api/core';
import type { ExecutionEvent } from '@/shared/types/ui/execution';
import { trackChannel } from '@/services/devHmrIpc';

export type ExecutionStreamDrain = {
  onmessage: (msg: ExecutionEvent) => void;
  waitForStreamEnd: (executedGraphs: number) => Promise<void>;
};

/** Channel event handler + post-invoke drain (testable without Tauri Channel). */
export function createExecutionStreamDrain(
  onEvent?: (event: ExecutionEvent) => void,
): ExecutionStreamDrain {
  let resolveEnd: (() => void) | undefined;
  const streamEnded = new Promise<void>((resolve) => {
    resolveEnd = resolve;
  });

  return {
    onmessage: (msg) => {
      onEvent?.(msg);
      if (msg.event === 'executionComplete') {
        queueMicrotask(() => resolveEnd?.());
      }
    },
    waitForStreamEnd: async (executedGraphs) => {
      if (executedGraphs > 0) {
        await streamEnded;
      }
    },
  };
}

export type ExecutionChannelBinding = {
  channel: Channel<ExecutionEvent>;
  waitForStreamEnd: (executedGraphs: number) => Promise<void>;
};

/**
 * `invoke("execute_project")` can resolve before the webview drains the Channel
 * queue. Wait until `executionComplete` is handled so callers see a full recording.
 */
export function bindExecutionEventChannel(
  onEvent?: (event: ExecutionEvent) => void,
): ExecutionChannelBinding {
  const drain = createExecutionStreamDrain(onEvent);
  const channel = trackChannel(new Channel<ExecutionEvent>());
  channel.onmessage = drain.onmessage;
  return { channel, waitForStreamEnd: drain.waitForStreamEnd };
}
