import { parseLogBatchDto } from "@/shared/types/dto/logParser";
import { describe, expect, it, vi } from "vitest";
import type { LogBatchDto, LogRecordDto, LogSubscriptionDto } from "@/shared/types/dto/log";
import { createRecordBatchReceiver, RecordStreamDiscontinuityError } from "./recordBatchReceiver";

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

const snapshot = (latestSequence: number): LogSubscriptionDto => ({
  subscriptionId: "subscription-1",
  streamId: "stream-1",
  entries: latestSequence === 0 ? [] : [record(latestSequence)],
  latestSequence,
  truncated: latestSequence > 1,
});

describe("recordBatchReceiver", () => {
  it("stops log delivery on storage failure without advancing the stream", () => {
    const received = vi.fn();
    const onError = vi.fn();
    const receiver = createRecordBatchReceiver(parseLogBatchDto, received, onError);
    expect(receiver.prepare(snapshot(1))).toBeNull();
    receiver.activate();
    receiver.onmessage({ streamId: "stream-1", entries: [], failure: "storage_unavailable" });
    receiver.onmessage(batch(2));
    expect(received).not.toHaveBeenCalled();
    expect(onError).toHaveBeenCalledWith(
      expect.objectContaining({ reason: "storage-unavailable" }),
    );
    expect(() => parseLogBatchDto({ ...batch(2), failure: "storage_unavailable" })).toThrow();
  });
  it("reports preactivation overflow instead of dropping old batches", () => {
    const receiver = createRecordBatchReceiver(parseLogBatchDto, vi.fn(), vi.fn(), 2);
    receiver.onmessage(batch(1));
    receiver.onmessage(batch(2));
    receiver.onmessage(batch(3));

    expect(receiver.prepare(snapshot(0))).toBe("preactivation-overflow");
    expect(() => receiver.activate()).toThrow(RecordStreamDiscontinuityError);
  });

  it("reports a preactivation sequence gap so the service can reconnect", () => {
    const receiver = createRecordBatchReceiver(parseLogBatchDto, vi.fn(), vi.fn(), 2);
    receiver.onmessage(batch(2));

    expect(receiver.prepare(snapshot(0))).toBe("sequence-gap");
  });

  it("stops an active receiver at a sequence gap until snapshot recovery", () => {
    const received = vi.fn();
    const onError = vi.fn();
    const receiver = createRecordBatchReceiver(parseLogBatchDto, received, onError, 2);
    expect(receiver.prepare(snapshot(1))).toBeNull();
    receiver.activate();

    receiver.onmessage(batch(3));

    receiver.onmessage(batch(4));
    expect(received).not.toHaveBeenCalled();
    expect(onError).toHaveBeenCalledWith(
      expect.objectContaining({
        reason: "sequence-gap",
      }),
    );
  });
});
