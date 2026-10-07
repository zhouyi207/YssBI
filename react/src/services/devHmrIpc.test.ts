import type { Channel } from "@tauri-apps/api/core";
import { describe, expect, it, vi } from "vitest";
import { disposeTrackedChannelsForHmr, trackChannel, untrackChannel } from "./devHmrIpc";

describe("dev HMR IPC disposal", () => {
  it("finishes the captured cleanup batch after failure and preserves a reentrant successor", () => {
    const failure = new Error("cleanup failed");
    const first = { onmessage: vi.fn() } as unknown as Channel<unknown>;
    const second = { onmessage: vi.fn() } as unknown as Channel<unknown>;
    const successor = { onmessage: vi.fn() } as unknown as Channel<unknown>;
    const firstHandler = first.onmessage;
    const secondHandler = second.onmessage;
    const successorHandler = successor.onmessage;
    const disposeSecond = vi.fn();
    const disposeSuccessor = vi.fn();
    trackChannel(first, () => {
      trackChannel(successor, disposeSuccessor);
      throw failure;
    });
    trackChannel(second, disposeSecond);
    try {
      expect(() => disposeTrackedChannelsForHmr()).toThrow(failure);
      expect(disposeSecond).toHaveBeenCalledOnce();
      expect(disposeSuccessor).not.toHaveBeenCalled();
      expect(first.onmessage).not.toBe(firstHandler);
      expect(second.onmessage).not.toBe(secondHandler);
      expect(successor.onmessage).toBe(successorHandler);

      disposeTrackedChannelsForHmr();
      disposeTrackedChannelsForHmr();
      expect(disposeSecond).toHaveBeenCalledOnce();
      expect(disposeSuccessor).toHaveBeenCalledOnce();
      expect(successor.onmessage).not.toBe(successorHandler);
    } finally {
      untrackChannel(first);
      untrackChannel(second);
      untrackChannel(successor);
    }
  });

  it("settles a tracked drain before replacing its channel handler", () => {
    const onmessage = vi.fn();
    const disposeDrain = vi.fn();
    const channel = { onmessage } as unknown as Channel<unknown>;
    trackChannel(channel, disposeDrain);

    disposeTrackedChannelsForHmr();
    disposeTrackedChannelsForHmr();
    (channel as unknown as { onmessage: (message: unknown) => void }).onmessage("late");

    expect(disposeDrain).toHaveBeenCalledOnce();
    expect(onmessage).not.toHaveBeenCalled();
  });
});
