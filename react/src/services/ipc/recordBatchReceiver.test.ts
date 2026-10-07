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
  it("queues reentrant batches behind pending records and stops when the consumer disposes", () => {
    const received: number[] = [];
    const onError = vi.fn();
    const receiver = createRecordBatchReceiver(
      parseLogBatchDto,
      (next) => {
        const sequence = next.entries[0].sequence;
        received.push(sequence);
        if (sequence === 1) {
          receiver.onmessage(batch(4));
          receiver.onmessage(batch(5));
        }
        if (sequence === 2) receiver.dispose();
      },
      onError,
    );
    receiver.onmessage(batch(1));
    receiver.onmessage(batch(2));
    receiver.onmessage(batch(3));
    expect(receiver.prepare(snapshot(0))).toBeNull();

    receiver.activate();
    receiver.onmessage(batch(6));

    expect(received).toEqual([1, 2]);
    expect(onError).not.toHaveBeenCalled();
  });

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
    expect(() => parseLogBatchDto({ ...batch(2), failure: "subscriber_lagged" })).toThrow();
  });
  it("reports preactivation overflow instead of dropping old batches", () => {
    const receiver = createRecordBatchReceiver(parseLogBatchDto, vi.fn(), vi.fn(), 2);
    receiver.onmessage(batch(1));
    receiver.onmessage(batch(2));
    receiver.onmessage(batch(3));

    expect(receiver.prepare(snapshot(0))).toBe("preactivation-overflow");
    expect(() => receiver.activate()).toThrow(RecordStreamDiscontinuityError);

    const received: number[] = [];
    const onError = vi.fn();
    const active = createRecordBatchReceiver(
      parseLogBatchDto,
      (next) => {
        received.push(next.entries[0].sequence);
        for (let sequence = 2; sequence <= 4; sequence++) active.onmessage(batch(sequence));
      },
      onError,
      2,
    );
    expect(active.prepare(snapshot(0))).toBeNull();
    active.activate();
    active.onmessage(batch(1));
    active.onmessage(batch(5));
    expect(received).toEqual([1]);
    expect(onError).toHaveBeenCalledWith(expect.objectContaining({ reason: "subscriber-lagged" }));
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
