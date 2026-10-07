import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Channel } from "@tauri-apps/api/core";
import type { LogBatchDto, LogRecordDto, FrontendLogEntryDto } from "@/shared/types/dto/log";

const core = vi.hoisted(() => ({
  invoke: vi.fn(),
  channels: [] as Array<{ onmessage?: (value: unknown) => void }>,
}));
const channelTracking = vi.hoisted(() => ({
  track: vi.fn((channel: unknown) => channel),
  untrack: vi.fn(),
  clear: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: core.invoke,
  Channel: class TestChannel<T> {
    onmessage?: (value: T) => void;

    constructor() {
      core.channels.push(this as { onmessage?: (value: unknown) => void });
    }
  },
}));

vi.mock("@/services/devHmrIpc", () => ({
  trackChannel: channelTracking.track,
  untrackChannel: channelTracking.untrack,
}));

vi.mock("@/shared/platform/tauriWebview", () => ({
  clearChannelMessageHandler: channelTracking.clear,
}));

import { LogService } from "./logService";

function record(sequence: number): LogRecordDto {
  return {
    streamId: "stream-1",
    sequence,
    timestamp: "2026-08-16T10:11:12.000",
    level: "info",
    origin: "rust",
    domain: "application",
    target: "app",
    message: `entry-${sequence}`,
    fields: {},
  };
}

const batch = (sequence: number): LogBatchDto => ({
  streamId: "stream-1",
  entries: [record(sequence)],
});

beforeEach(() => {
  core.invoke.mockReset();
  core.channels.length = 0;
  channelTracking.track.mockClear();
  channelTracking.untrack.mockClear();
  channelTracking.clear.mockClear();
});

describe("LogService plugin contract", () => {
  it("submits a frontend batch with the fixed command payload", async () => {
    core.invoke.mockResolvedValue(undefined);
    const entries: FrontendLogEntryDto[] = [
      {
        level: "warn",
        domain: "graph",
        target: "GraphManagement",
        message: "failed",
        fields: { retryable: false },
      },
    ];

    await LogService.submitFrontendLogs(entries);

    expect(core.invoke).toHaveBeenCalledWith("plugin:tracing|submit_frontend_logs", { entries });
  });

  it("applies the initial snapshot before draining early Channel batches", async () => {
    const received: LogBatchDto[] = [];
    core.invoke.mockImplementation(async (command: string, args?: unknown) => {
      if (command !== "plugin:tracing|subscribe_logs") return undefined;
      const channel = (args as { onRecords: Channel<unknown> }).onRecords;
      channel.onmessage?.(batch(2));
      return {
        subscriptionId: "subscription-1",
        streamId: "stream-1",
        entries: [record(1)],
        latestSequence: 1,
        truncated: false,
      };
    });

    const subscription = await LogService.subscribeLogs((next) => received.push(next));
    expect(subscription.snapshot.entries).toEqual([record(1)]);
    expect(received).toEqual([]);

    subscription.activate();
    expect(received).toEqual([batch(2)]);
    core.channels[0]?.onmessage?.(batch(3));
    expect(received).toEqual([batch(2), batch(3)]);
  });

  it("reconnects instead of silently dropping preactivation overflow", async () => {
    let subscribeAttempt = 0;
    core.invoke.mockImplementation(async (command: string, args?: unknown) => {
      if (command === "plugin:tracing|unsubscribe_logs") return undefined;
      if (command !== "plugin:tracing|subscribe_logs") return undefined;
      subscribeAttempt += 1;
      const channel = (args as { onRecords: Channel<unknown> }).onRecords;
      if (subscribeAttempt === 1) {
        for (let sequence = 1; sequence <= 65; sequence += 1) {
          channel.onmessage?.(batch(sequence));
        }
        return {
          subscriptionId: "subscription-overflowed",
          streamId: "stream-1",
          entries: [],
          latestSequence: 0,
          truncated: false,
        };
      }
      return {
        subscriptionId: "subscription-reconnected",
        streamId: "stream-1",
        entries: [record(65)],
        latestSequence: 65,
        truncated: true,
      };
    });

    const subscription = await LogService.subscribeLogs(vi.fn());

    expect(subscription.snapshot.subscriptionId).toBe("subscription-reconnected");
    expect(
      core.invoke.mock.calls.filter(([command]) => command === "plugin:tracing|subscribe_logs"),
    ).toHaveLength(2);
    expect(core.invoke).toHaveBeenCalledWith("plugin:tracing|unsubscribe_logs", {
      subscriptionId: "subscription-overflowed",
    });
  });

  it("releases a lagged subscription and resumes from a fresh snapshot", async () => {
    let subscribeAttempt = 0;
    core.invoke.mockImplementation(async (command: string) => {
      if (command !== "plugin:tracing|subscribe_logs") return undefined;
      subscribeAttempt += 1;
      return {
        subscriptionId: `subscription-${subscribeAttempt}`,
        streamId: "stream-1",
        entries: subscribeAttempt === 1 ? [record(1)] : [record(1), record(2), record(3)],
        latestSequence: subscribeAttempt === 1 ? 1 : 3,
        truncated: false,
      };
    });
    const received = vi.fn();
    const onDiscontinuity = vi.fn();
    const subscription = await LogService.subscribeLogs(received, onDiscontinuity);
    subscription.activate();
    const oldChannel = core.channels[0];
    oldChannel?.onmessage?.({
      streamId: "stream-1",
      entries: [],
      failure: "subscriber_lagged",
    });

    expect(onDiscontinuity).toHaveBeenCalledWith(
      expect.objectContaining({ reason: "subscriber-lagged" }),
    );
    expect(channelTracking.untrack).toHaveBeenCalledExactlyOnceWith(oldChannel);
    expect(channelTracking.clear).toHaveBeenCalledExactlyOnceWith(oldChannel);
    await subscription.unsubscribe();
    expect(
      core.invoke.mock.calls.filter(([command]) => command === "plugin:tracing|unsubscribe_logs"),
    ).toEqual([["plugin:tracing|unsubscribe_logs", { subscriptionId: "subscription-1" }]]);

    const recovered = await LogService.subscribeLogs(received, onDiscontinuity);
    expect(recovered.snapshot.entries).toEqual([record(1), record(2), record(3)]);
    recovered.activate();
    oldChannel?.onmessage?.(batch(2));
    core.channels[1]?.onmessage?.(batch(4));
    expect(received).toHaveBeenCalledExactlyOnceWith(batch(4));
    expect(onDiscontinuity).toHaveBeenCalledOnce();
    await recovered.unsubscribe();
  });

  it("unsubscribes once and detaches the Channel handler", async () => {
    core.invoke.mockResolvedValue({
      subscriptionId: "subscription-1",
      streamId: "stream-1",
      entries: [],
      latestSequence: 0,
      truncated: false,
    });
    const subscription = await LogService.subscribeLogs(vi.fn());

    core.invoke.mockResolvedValue(undefined);
    await subscription.unsubscribe();
    await subscription.unsubscribe();

    expect(core.invoke).toHaveBeenCalledWith("plugin:tracing|unsubscribe_logs", {
      subscriptionId: "subscription-1",
    });
    expect(
      core.invoke.mock.calls.filter(([command]) => command === "plugin:tracing|unsubscribe_logs"),
    ).toHaveLength(1);
    expect(channelTracking.untrack).toHaveBeenCalledOnce();
    expect(channelTracking.clear).toHaveBeenCalledOnce();
  });
});
