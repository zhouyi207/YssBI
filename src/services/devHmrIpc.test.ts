import type { Channel } from "@tauri-apps/api/core";
import { describe, expect, it, vi } from "vitest";
import { disposeTrackedChannelsForHmr, trackChannel } from "./devHmrIpc";

describe("dev HMR IPC disposal", () => {
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
