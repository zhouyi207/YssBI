import { describe, expect, it, vi } from "vitest";
import type { LogRecordDto } from "@/shared/types/domain/log";
import { createLogLogBuffer, type LogSnapshot } from "./logBuffer";

function record(
  sequence: number,
  streamId = "stream-1",
  message = `entry-${sequence}`,
): LogRecordDto {
  return {
    streamId,
    sequence,
    timestamp: `2026-08-16T10:11:${String(sequence).padStart(2, "0")}.000Z`,
    level: "info",
    origin: "rust",
    domain: "execution",
    target: "executor",
    message,
    fields: {},
  };
}

describe("diagnostic logBuffer", () => {
  it("retains committed records and unaffected domain views when a subscription recovers", () => {
    const buffer = createLogLogBuffer(3);
    const initial = {
      subscriptionId: "subscription-1",
      streamId: "stream-1",
      entries: [{ ...record(1), domain: "graph" as const }, record(2)],
      latestSequence: 2,
      truncated: false,
    };
    buffer.setSubscription(initial);
    const before = buffer.getSnapshot();
    const listener = vi.fn();
    buffer.subscribe(listener);
    buffer.setSubscription({ ...structuredClone(initial), subscriptionId: "subscription-2" });
    expect(buffer.getSnapshot()).toBe(before);
    expect(listener).not.toHaveBeenCalled();

    buffer.setSubscription({
      ...structuredClone(initial),
      subscriptionId: "subscription-3",
      entries: [...structuredClone(initial.entries), record(3)],
      latestSequence: 3,
    });
    const after = buffer.getSnapshot();
    expect(listener).toHaveBeenCalledOnce();
    expect(after.entries[0]).toBe(before.entries[0]);
    expect(after.entries[1]).toBe(before.entries[1]);
    expect(after.entriesByDomain.graph).toBe(before.entriesByDomain.graph);
    expect(after.entriesByDomain.execution).toEqual([before.entries[1], record(3)]);
    expect(after.entriesByDomain.all).toBe(after.entries);
    expect(before.entries).toEqual(initial.entries);
  });

  it("restores cleared history and gap state while keeping record identities scoped to the stream", () => {
    const buffer = createLogLogBuffer(3);
    const initial = {
      subscriptionId: "subscription-1",
      streamId: "stream-1",
      entries: [record(1), record(2)],
      latestSequence: 2,
      truncated: false,
    };
    buffer.setSubscription(initial);
    const before = buffer.getSnapshot();
    buffer.markTruncated();
    buffer.setSubscription(structuredClone(initial));
    expect(buffer.getSnapshot().truncated).toBe(false);
    expect(buffer.getSnapshot().entries).toBe(before.entries);
    buffer.clear();
    buffer.setSubscription(structuredClone(initial));
    expect(buffer.getSnapshot().entries).toEqual(initial.entries);
    expect(buffer.getSnapshot().latestSequence).toBe(2);

    buffer.setSubscription({
      ...initial,
      subscriptionId: "subscription-2",
      streamId: "stream-2",
      entries: [record(1, "stream-2", "new stream"), record(2, "stream-2")],
    });
    const replaced = buffer.getSnapshot();
    expect(replaced.entries[0].message).toBe("new stream");
    expect(replaced.entries[0]).not.toBe(before.entries[0]);
    expect(replaced.truncated).toBe(true);
    expect(before.entries[0].streamId).toBe("stream-1");
  });

  it("publishes bounded domain indexes with the records and preserves unaffected domains", () => {
    const buffer = createLogLogBuffer(3);
    const graph = { ...record(1), domain: "graph" as const };
    buffer.setSubscription({
      subscriptionId: "subscription-1",
      streamId: "stream-1",
      entries: [graph, record(2)],
      latestSequence: 2,
      truncated: false,
    });
    const initial = buffer.getSnapshot();
    expect(initial.entriesByDomain.all).toBe(initial.entries);
    expect(initial.entriesByDomain.graph).toEqual([graph]);
    const published: LogSnapshot[] = [];
    buffer.subscribe(() => published.push(buffer.getSnapshot()));

    buffer.appendBatch({ streamId: "stream-1", entries: [record(3)] });
    expect(published).toHaveLength(1);
    expect(published[0].entriesByDomain.graph).toBe(initial.entriesByDomain.graph);
    expect(published[0].entriesByDomain.execution.map((entry) => entry.sequence)).toEqual([2, 3]);

    buffer.appendBatch({ streamId: "stream-1", entries: [record(4)] });
    expect(buffer.getSnapshot().entriesByDomain.graph).toEqual([]);
    expect(buffer.getSnapshot().entriesByDomain.execution.map((entry) => entry.sequence)).toEqual([
      2, 3, 4,
    ]);
    buffer.appendBatch({
      streamId: "stream-1",
      entries: [record(5), { ...record(6), domain: "graph" }, record(7), record(8)],
    });
    expect(buffer.getSnapshot().entries.map((entry) => entry.sequence)).toEqual([6, 7, 8]);
    expect(buffer.getSnapshot().entriesByDomain.graph.map((entry) => entry.sequence)).toEqual([6]);
    expect(buffer.getSnapshot().entriesByDomain.execution.map((entry) => entry.sequence)).toEqual([
      7, 8,
    ]);
    buffer.setSubscription({
      subscriptionId: "subscription-2",
      streamId: "stream-2",
      entries: [{ ...record(1, "stream-2"), domain: "graph" }],
      latestSequence: 1,
      truncated: false,
    });
    expect(buffer.getSnapshot().entriesByDomain.execution).toEqual([]);
    expect(buffer.getSnapshot().entriesByDomain.graph[0].streamId).toBe("stream-2");
    buffer.clear();
    expect(buffer.getSnapshot().latestSequence).toBe(1);
    expect(
      Object.values(buffer.getSnapshot().entriesByDomain).every((entries) => !entries.length),
    ).toBe(true);
    expect(published).toHaveLength(5);
    for (const snapshot of published) {
      expect(snapshot.entriesByDomain.all).toBe(snapshot.entries);
      for (const entry of snapshot.entries) {
        expect(snapshot.entriesByDomain[entry.domain]).toContain(entry);
      }
    }
  });

  it("sorts, deduplicates, and bounds an initial stream snapshot", () => {
    const buffer = createLogLogBuffer(2);
    buffer.setSubscription({
      subscriptionId: "subscription-1",
      streamId: "stream-1",
      entries: [record(3), record(1), record(2), record(2, "stream-1", "replacement")],
      latestSequence: 3,
      truncated: false,
    });

    expect(buffer.getSnapshot()).toMatchObject({
      streamId: "stream-1",
      latestSequence: 3,
      truncated: true,
    });
    expect(buffer.getSnapshot().entries.map((entry) => [entry.sequence, entry.message])).toEqual([
      [2, "replacement"],
      [3, "entry-3"],
    ]);
  });

  it("appends only newer sequences and emits once per accepted batch", () => {
    const buffer = createLogLogBuffer(3);
    const listener = vi.fn();
    buffer.subscribe(listener);
    buffer.setSubscription({
      subscriptionId: "subscription-1",
      streamId: "stream-1",
      entries: [record(1), record(2)],
      latestSequence: 2,
      truncated: false,
    });

    buffer.appendBatch({
      streamId: "stream-1",
      entries: [record(2, "stream-1", "duplicate"), record(4), record(3), record(4)],
    });

    expect(buffer.getSnapshot().entries.map((entry) => entry.sequence)).toEqual([2, 3, 4]);
    expect(buffer.getSnapshot()).toMatchObject({ latestSequence: 4, truncated: true });
    expect(listener).toHaveBeenCalledTimes(2);
  });

  it("resets recent entries when the backend stream changes", () => {
    const buffer = createLogLogBuffer(3);
    buffer.setSubscription({
      subscriptionId: "subscription-1",
      streamId: "stream-1",
      entries: [record(8)],
      latestSequence: 8,
      truncated: false,
    });
    buffer.appendBatch({ streamId: "stream-2", entries: [record(1, "stream-2")] });

    expect(buffer.getSnapshot()).toMatchObject({
      streamId: "stream-2",
      latestSequence: 1,
      truncated: true,
    });
    expect(buffer.getSnapshot().entries.map((entry) => entry.sequence)).toEqual([1]);
  });

  it("marks sequence gaps as truncated instead of silently advancing", () => {
    const buffer = createLogLogBuffer(10);
    buffer.setSubscription({
      subscriptionId: "subscription-1",
      streamId: "stream-1",
      entries: [record(1)],
      latestSequence: 1,
      truncated: false,
    });

    buffer.appendBatch({ streamId: "stream-1", entries: [record(3)] });

    expect(buffer.getSnapshot()).toMatchObject({
      latestSequence: 3,
      truncated: true,
    });
    expect(buffer.getSnapshot().entries.map((entry) => entry.sequence)).toEqual([1, 3]);
  });

  it("keeps the sequence watermark when the local recent view is cleared", () => {
    const buffer = createLogLogBuffer(3);
    buffer.setSubscription({
      subscriptionId: "subscription-1",
      streamId: "stream-1",
      entries: [record(5)],
      latestSequence: 5,
      truncated: false,
    });
    buffer.clear();
    buffer.appendBatch({ streamId: "stream-1", entries: [record(5)] });
    expect(buffer.getSnapshot().entries).toEqual([]);

    buffer.appendBatch({ streamId: "stream-1", entries: [record(6)] });
    expect(buffer.getSnapshot().entries.map((entry) => entry.sequence)).toEqual([6]);
  });
});
