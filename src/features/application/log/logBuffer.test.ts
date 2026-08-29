import { describe, expect, it, vi } from 'vitest';
import type { DiagnosticRecordDto } from '@/shared/types/domain/diagnostics';
import { createDiagnosticLogBuffer } from './logBuffer';

function record(sequence: number, streamId = 'stream-1', message = `entry-${sequence}`): DiagnosticRecordDto {
  return {
    streamId,
    sequence,
    timestamp: `2026-08-16T10:11:${String(sequence).padStart(2, '0')}.000Z`,
    level: 'info',
    origin: 'rust',
    domain: 'execution',
    target: 'executor',
    message,
    fields: {},
  };
}

describe('diagnostic logBuffer', () => {
  it('sorts, deduplicates, and bounds an initial stream snapshot', () => {
    const buffer = createDiagnosticLogBuffer(2);
    buffer.setSubscription({
      subscriptionId: 'subscription-1',
      streamId: 'stream-1',
      entries: [record(3), record(1), record(2), record(2, 'stream-1', 'replacement')],
      latestSequence: 3,
      truncated: false,
    });

    expect(buffer.getSnapshot()).toMatchObject({
      streamId: 'stream-1',
      latestSequence: 3,
      truncated: true,
    });
    expect(buffer.getSnapshot().entries.map((entry) => [entry.sequence, entry.message]))
      .toEqual([[2, 'replacement'], [3, 'entry-3']]);
  });

  it('appends only newer sequences and emits once per accepted batch', () => {
    const buffer = createDiagnosticLogBuffer(3);
    const listener = vi.fn();
    buffer.subscribe(listener);
    buffer.setSubscription({
      subscriptionId: 'subscription-1',
      streamId: 'stream-1',
      entries: [record(1), record(2)],
      latestSequence: 2,
      truncated: false,
    });

    buffer.appendBatch({
      streamId: 'stream-1',
      entries: [record(2, 'stream-1', 'duplicate'), record(4), record(3), record(4)],
    });

    expect(buffer.getSnapshot().entries.map((entry) => entry.sequence)).toEqual([2, 3, 4]);
    expect(buffer.getSnapshot()).toMatchObject({ latestSequence: 4, truncated: true });
    expect(listener).toHaveBeenCalledTimes(2);
  });

  it('resets recent entries when the backend stream changes', () => {
    const buffer = createDiagnosticLogBuffer(3);
    buffer.setSubscription({
      subscriptionId: 'subscription-1',
      streamId: 'stream-1',
      entries: [record(8)],
      latestSequence: 8,
      truncated: false,
    });
    buffer.appendBatch({ streamId: 'stream-2', entries: [record(1, 'stream-2')] });

    expect(buffer.getSnapshot()).toMatchObject({
      streamId: 'stream-2',
      latestSequence: 1,
      truncated: true,
    });
    expect(buffer.getSnapshot().entries.map((entry) => entry.sequence)).toEqual([1]);
  });

  it('marks sequence gaps as truncated instead of silently advancing', () => {
    const buffer = createDiagnosticLogBuffer(10);
    buffer.setSubscription({
      subscriptionId: 'subscription-1',
      streamId: 'stream-1',
      entries: [record(1)],
      latestSequence: 1,
      truncated: false,
    });

    buffer.appendBatch({ streamId: 'stream-1', entries: [record(3)] });

    expect(buffer.getSnapshot()).toMatchObject({
      latestSequence: 3,
      truncated: true,
    });
    expect(buffer.getSnapshot().entries.map((entry) => entry.sequence)).toEqual([1, 3]);
  });

  it('keeps the sequence watermark when the local recent view is cleared', () => {
    const buffer = createDiagnosticLogBuffer(3);
    buffer.setSubscription({
      subscriptionId: 'subscription-1',
      streamId: 'stream-1',
      entries: [record(5)],
      latestSequence: 5,
      truncated: false,
    });
    buffer.clear();
    buffer.appendBatch({ streamId: 'stream-1', entries: [record(5)] });
    expect(buffer.getSnapshot().entries).toEqual([]);

    buffer.appendBatch({ streamId: 'stream-1', entries: [record(6)] });
    expect(buffer.getSnapshot().entries.map((entry) => entry.sequence)).toEqual([6]);
  });
});
