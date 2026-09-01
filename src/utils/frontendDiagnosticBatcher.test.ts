import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { FrontendDiagnosticEntryDto } from "@/shared/types/domain/diagnostics";
import { createFrontendDiagnosticBatcher } from "./frontendDiagnosticBatcher";

function entry(message: string): FrontendDiagnosticEntryDto {
  return {
    level: "info",
    domain: "application",
    target: "test",
    message,
    fields: {},
  };
}

function deferred() {
  let resolve!: () => void;
  const promise = new Promise<void>((next) => {
    resolve = next;
  });
  return { promise, resolve };
}

describe("createFrontendDiagnosticBatcher", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it("flushes one bounded batch after the maximum delay", async () => {
    const submit = vi.fn().mockResolvedValue(undefined);
    const batcher = createFrontendDiagnosticBatcher({
      maxBatchEntries: 3,
      maxPendingEntries: 6,
      maxDelayMs: 50,
      maxMessageBytes: 100,
      submit,
    });

    batcher.enqueue(entry("one"));
    batcher.enqueue(entry("two"));
    await vi.advanceTimersByTimeAsync(49);
    expect(submit).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(1);

    expect(submit).toHaveBeenCalledOnce();
    expect(
      submit.mock.calls[0]?.[0].map((item: FrontendDiagnosticEntryDto) => item.message),
    ).toEqual(["one", "two"]);
    batcher.dispose();
  });

  it("flushes at the batch limit and truncates oversized messages", async () => {
    const submit = vi.fn().mockResolvedValue(undefined);
    const batcher = createFrontendDiagnosticBatcher({
      maxBatchEntries: 2,
      maxPendingEntries: 4,
      maxDelayMs: 100,
      maxMessageBytes: 5,
      submit,
    });

    batcher.enqueue(entry("abcdef"));
    batcher.enqueue(entry("two"));
    await batcher.flush();

    expect(submit.mock.calls[0]?.[0]).toMatchObject([{ message: "ab…" }, { message: "two" }]);
    expect(
      new TextEncoder().encode(submit.mock.calls[0]?.[0][0].message).byteLength,
    ).toBeLessThanOrEqual(5);
    batcher.dispose();
  });

  it("keeps only the newest pending entries while a submission is in flight", async () => {
    const first = deferred();
    const submit = vi
      .fn()
      .mockImplementationOnce(() => first.promise)
      .mockResolvedValue(undefined);
    const batcher = createFrontendDiagnosticBatcher({
      maxBatchEntries: 1,
      maxPendingEntries: 3,
      maxDelayMs: 100,
      maxMessageBytes: 100,
      submit,
    });

    batcher.enqueue(entry("in-flight"));
    batcher.enqueue(entry("drop-me"));
    batcher.enqueue(entry("keep-1"));
    batcher.enqueue(entry("keep-2"));
    batcher.enqueue(entry("keep-3"));
    expect(batcher.pendingCount()).toBe(3);

    first.resolve();
    await batcher.flush();
    expect(submit.mock.calls.slice(1).map((call) => call[0][0].message)).toEqual([
      "keep-1",
      "keep-2",
      "keep-3",
    ]);
    batcher.dispose();
  });

  it("drops a failed batch without retrying or blocking later entries", async () => {
    const submit = vi
      .fn()
      .mockRejectedValueOnce(new Error("transport unavailable"))
      .mockResolvedValue(undefined);
    const batcher = createFrontendDiagnosticBatcher({
      maxBatchEntries: 1,
      maxPendingEntries: 2,
      maxDelayMs: 10,
      maxMessageBytes: 100,
      submit,
    });

    batcher.enqueue(entry("failed"));
    await batcher.flush();
    batcher.enqueue(entry("later"));
    await batcher.flush();

    expect(submit).toHaveBeenCalledTimes(2);
    expect(submit.mock.calls[1]?.[0]).toMatchObject([{ message: "later" }]);
    batcher.dispose();
  });
});
