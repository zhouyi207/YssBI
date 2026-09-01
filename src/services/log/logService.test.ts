import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Channel } from "@tauri-apps/api/core";
import type {
  DiagnosticBatchDto,
  DiagnosticRecordDto,
  FrontendDiagnosticEntryDto,
} from "@/shared/types/dto/diagnostics";

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

function record(sequence: number): DiagnosticRecordDto {
  return {
    streamId: "stream-1",
    sequence,
    timestamp: "2026-08-16T10:11:12.000Z",
    level: "info",
    origin: "rust",
    domain: "application",
    target: "app",
    message: `entry-${sequence}`,
    fields: {},
  };
}

const batch = (sequence: number): DiagnosticBatchDto => ({
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

describe("LogService diagnostics contract", () => {
  it("submits a frontend batch with the fixed command payload", async () => {
    core.invoke.mockResolvedValue(undefined);
    const entries: FrontendDiagnosticEntryDto[] = [
      {
        level: "warn",
        domain: "graph",
        target: "GraphManagement",
        message: "failed",
        fields: { retryable: false },
      },
    ];

    await LogService.submitFrontendDiagnostics(entries);

    expect(core.invoke).toHaveBeenCalledWith("submit_frontend_diagnostics", { entries });
  });

  it("applies the initial snapshot before draining early Channel batches", async () => {
    const received: DiagnosticBatchDto[] = [];
    core.invoke.mockImplementation(async (command: string, args?: unknown) => {
      if (command !== "subscribe_diagnostics") return undefined;
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

    const subscription = await LogService.subscribeDiagnostics((next) => received.push(next));
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
      if (command === "unsubscribe_diagnostics") return undefined;
      if (command !== "subscribe_diagnostics") return undefined;
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

    const subscription = await LogService.subscribeDiagnostics(vi.fn());

    expect(subscription.snapshot.subscriptionId).toBe("subscription-reconnected");
    expect(
      core.invoke.mock.calls.filter(([command]) => command === "subscribe_diagnostics"),
    ).toHaveLength(2);
    expect(core.invoke).toHaveBeenCalledWith("unsubscribe_diagnostics", {
      subscriptionId: "subscription-overflowed",
    });
  });

  it("unsubscribes once and detaches the Channel handler", async () => {
    core.invoke.mockResolvedValue({
      subscriptionId: "subscription-1",
      streamId: "stream-1",
      entries: [],
      latestSequence: 0,
      truncated: false,
    });
    const subscription = await LogService.subscribeDiagnostics(vi.fn());

    core.invoke.mockResolvedValue(undefined);
    await subscription.unsubscribe();
    await subscription.unsubscribe();

    expect(core.invoke).toHaveBeenCalledWith("unsubscribe_diagnostics", {
      subscriptionId: "subscription-1",
    });
    expect(
      core.invoke.mock.calls.filter(([command]) => command === "unsubscribe_diagnostics"),
    ).toHaveLength(1);
    expect(channelTracking.untrack).toHaveBeenCalledOnce();
    expect(channelTracking.clear).toHaveBeenCalledOnce();
  });
});
