import { describe, expect, it, vi } from "vitest";
import type {
  DiagnosticBatchDto,
  DiagnosticRecordDto,
  DiagnosticSubscriptionDto,
} from "@/shared/types/dto/diagnostics";
import {
  createDiagnosticBatchReceiver,
  DiagnosticStreamDiscontinuityError,
} from "./diagnosticBatchReceiver";

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

const snapshot = (latestSequence: number): DiagnosticSubscriptionDto => ({
  subscriptionId: "subscription-1",
  streamId: "stream-1",
  entries: latestSequence === 0 ? [] : [record(latestSequence)],
  latestSequence,
  truncated: latestSequence > 1,
});

describe("diagnosticBatchReceiver", () => {
  it("reports preactivation overflow instead of dropping old batches", () => {
    const receiver = createDiagnosticBatchReceiver(vi.fn(), vi.fn(), 2);
    receiver.onmessage(batch(1));
    receiver.onmessage(batch(2));
    receiver.onmessage(batch(3));

    expect(receiver.prepare(snapshot(0))).toBe("preactivation-overflow");
    expect(() => receiver.activate()).toThrow(DiagnosticStreamDiscontinuityError);
  });

  it("reports a preactivation sequence gap so the service can reconnect", () => {
    const receiver = createDiagnosticBatchReceiver(vi.fn(), vi.fn(), 2);
    receiver.onmessage(batch(2));

    expect(receiver.prepare(snapshot(0))).toBe("sequence-gap");
  });

  it("stops an active receiver at a sequence gap until snapshot recovery", () => {
    const received = vi.fn();
    const onError = vi.fn();
    const receiver = createDiagnosticBatchReceiver(received, onError, 2);
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
