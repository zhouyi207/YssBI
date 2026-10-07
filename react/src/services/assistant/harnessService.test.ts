import { expect, it, vi } from "vitest";
import { HarnessService } from "./harnessService";

const transport = vi.hoisted(() => ({
  invoke: vi.fn(),
  channels: [] as Array<{ onmessage?: (value: unknown) => void }>,
  dispose: undefined as (() => void) | undefined,
  clear: vi.fn(),
  untrack: vi.fn(),
}));
vi.mock("@tauri-apps/api/core", () => ({
  Channel: class {
    onmessage?: (value: unknown) => void;
    constructor() {
      transport.channels.push(this);
    }
  },
}));
vi.mock("@/services/ipc", () => ({ invokeCommand: transport.invoke }));
vi.mock("@/services/devHmrIpc", () => ({
  trackChannel: (channel: unknown, dispose: () => void) => {
    transport.dispose = dispose;
    return channel;
  },
  untrackChannel: transport.untrack,
}));
vi.mock("@/shared/platform/tauriWebview", () => ({
  clearChannelMessageHandler: transport.clear,
}));

it("rejects a late subscription reply and queued callbacks after HMR disposal", async () => {
  let reply!: (value: unknown) => void;
  transport.invoke.mockImplementation((command: string) => {
    if (command === "subscribe_harness_events") {
      return new Promise((resolve) => {
        reply = resolve;
      });
    }
    return Promise.resolve();
  });
  const onEvent = vi.fn();
  const onError = vi.fn();
  const pending = HarnessService.subscribeEvents("session-1", 0, onEvent, onError);
  const onmessage = transport.channels[0]?.onmessage;
  transport.dispose?.();
  onmessage?.({
    sessionId: "session-1",
    sequence: 1,
    turnId: null,
    occurredAt: 1000,
    type: "session_created",
  });
  const rejected = expect(pending).rejects.toThrow("disposed");
  reply({ subscriptionId: "late-subscription" });
  await rejected;

  expect(onEvent).not.toHaveBeenCalled();
  expect(onError).not.toHaveBeenCalled();
  expect(transport.untrack).toHaveBeenCalledOnce();
  expect(transport.clear).toHaveBeenCalledOnce();
  expect(
    transport.invoke.mock.calls.filter(([command]) => command === "unsubscribe_harness_events"),
  ).toEqual([["unsubscribe_harness_events", { subscriptionId: "late-subscription" }]]);
});
