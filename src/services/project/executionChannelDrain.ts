import { Channel } from "@tauri-apps/api/core";
import type {
  ExecutionChannelEvent,
  RunEvent,
  RunOutputChannelEvent,
} from "@/shared/types/dto/runEvent";
import { parseExecutionChannelEvent } from "@/shared/types/dto/runEventParser";
import { trackChannel } from "@/services/devHmrIpc";

export class ExecutionChannelDisposedError extends Error {
  readonly code = "execution_channel_disposed";

  constructor() {
    super("execution event channel was disposed before a terminal event");
    this.name = "ExecutionChannelDisposedError";
  }
}

export type ExecutionStreamDrain = {
  onmessage: (msg: unknown) => void;
  waitForStreamEnd: () => Promise<void>;
  dispose: () => void;
};

type CallbackError = { caught: unknown };
type StreamSettlement =
  | { reason: "terminal"; callbackError?: CallbackError }
  | { reason: "invalid"; caught: unknown }
  | { reason: "disposed" };

function deliverRunEvent(
  event: RunEvent,
  onEvent: ((event: RunEvent) => void) | undefined,
  settle: (settlement: StreamSettlement) => void,
): void {
  const terminal =
    event.kind.type === "runCompleted" ||
    event.kind.type === "runErrored" ||
    event.kind.type === "runCancelled";
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
    settle({ reason: "terminal", callbackError });
  }
}

function deliverExecutionChannelEvent(
  event: ExecutionChannelEvent,
  onEvent: ((event: RunEvent) => void) | undefined,
  onOutput: ((event: RunOutputChannelEvent) => void) | undefined,
  settle: (settlement: StreamSettlement) => void,
): void {
  if (!("kind" in event)) {
    onOutput?.(event);
    return;
  }
  deliverRunEvent(event, onEvent, settle);
}

/** Channel event handler + post-invoke drain (testable without Tauri Channel). */
export function createExecutionStreamDrain(
  onEvent?: (event: RunEvent) => void,
  onOutput?: (event: RunOutputChannelEvent) => void,
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
    onmessage: (raw) => {
      try {
        deliverExecutionChannelEvent(parseExecutionChannelEvent(raw), onEvent, onOutput, settle);
      } catch (caught) {
        settle({ reason: "invalid", caught });
      }
    },
    waitForStreamEnd: async () => {
      const settlement = await streamEnded;
      if (settlement.reason === "disposed") {
        throw new ExecutionChannelDisposedError();
      }
      if (settlement.reason === "invalid") throw settlement.caught;
      if (settlement.callbackError) {
        throw settlement.callbackError.caught;
      }
    },
    dispose: () => settle({ reason: "disposed" }),
  };
}

export type ExecutionChannelBinding = {
  channel: Channel<unknown>;
  waitForStreamEnd: () => Promise<void>;
};

/**
 * `invokeCommand("execute_graph_document")` can resolve before the webview drains the
 * Channel queue. Wait for a terminal run event so callers observe every queued event.
 */
export function bindExecutionEventChannel(
  onEvent?: (event: RunEvent) => void,
  onOutput?: (event: RunOutputChannelEvent) => void,
): ExecutionChannelBinding {
  const drain = createExecutionStreamDrain(onEvent, onOutput);
  const channel = trackChannel(new Channel<unknown>(), drain.dispose);
  channel.onmessage = drain.onmessage;
  return { channel, waitForStreamEnd: drain.waitForStreamEnd };
}
