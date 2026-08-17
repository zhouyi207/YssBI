import { LOG_BUFFER_MAX } from '@/app/appConfig/default';
import type {
  DiagnosticBatchDto,
  DiagnosticRecordDto,
  DiagnosticSubscriptionDto,
} from '@/shared/types/dto/diagnostics';

export interface LogSnapshot {
  streamId: string | null;
  entries: DiagnosticRecordDto[];
  latestSequence: number | null;
  truncated: boolean;
}

export interface DiagnosticLogBuffer {
  subscribe: (listener: () => void) => () => void;
  getSnapshot: () => LogSnapshot;
  setSubscription: (subscription: DiagnosticSubscriptionDto) => void;
  appendBatch: (batch: DiagnosticBatchDto) => void;
  clear: () => void;
}

interface RecentEntries {
  entries: DiagnosticRecordDto[];
  allSequences: number[];
  truncated: boolean;
}

function recentDistinctEntries(
  entries: readonly DiagnosticRecordDto[],
  streamId: string,
  maxEntries: number,
): RecentEntries {
  const bySequence = new Map<number, DiagnosticRecordDto>();
  for (const entry of entries) {
    if (entry.streamId === streamId) bySequence.set(entry.sequence, entry);
  }
  const ordered = [...bySequence.values()].sort((left, right) => left.sequence - right.sequence);
  const truncated = ordered.length > maxEntries;
  return {
    entries: truncated ? ordered.slice(ordered.length - maxEntries) : ordered,
    allSequences: ordered.map((entry) => entry.sequence),
    truncated,
  };
}

function hasSequenceGap(
  sequences: readonly number[],
  expectedFirst: number,
  expectedLast?: number,
): boolean {
  if (sequences.length === 0) return expectedLast !== undefined && expectedLast >= expectedFirst;
  let expected = expectedFirst;
  for (const sequence of sequences) {
    if (sequence !== expected) return true;
    expected = sequence + 1;
  }
  return expectedLast !== undefined && sequences[sequences.length - 1] !== expectedLast;
}

export function createDiagnosticLogBuffer(maxEntries = LOG_BUFFER_MAX): DiagnosticLogBuffer {
  if (!Number.isInteger(maxEntries) || maxEntries <= 0) {
    throw new Error('Diagnostic log buffer capacity must be a positive integer');
  }

  let streamId: string | null = null;
  let entries: DiagnosticRecordDto[] = [];
  let latestSequence: number | null = null;
  let truncated = false;
  let snapshot: LogSnapshot = { streamId, entries, latestSequence, truncated };
  const listeners = new Set<() => void>();

  const publish = () => {
    snapshot = {
      streamId,
      entries: [...entries],
      latestSequence,
      truncated,
    };
    for (const listener of listeners) listener();
  };

  return {
    subscribe: (listener) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    getSnapshot: () => snapshot,
    setSubscription: (subscription) => {
      const recent = recentDistinctEntries(subscription.entries, subscription.streamId, maxEntries);
      const snapshotGap = hasSequenceGap(
        recent.allSequences,
        subscription.truncated
          ? (recent.allSequences[0] ?? subscription.latestSequence + 1)
          : 1,
        subscription.latestSequence,
      );
      streamId = subscription.streamId;
      entries = recent.entries;
      latestSequence = subscription.latestSequence;
      truncated = subscription.truncated || recent.truncated || snapshotGap;
      publish();
    },
    appendBatch: (batch) => {
      const sameStream = streamId === batch.streamId;
      const watermark = sameStream ? (latestSequence ?? 0) : 0;
      const recentIncoming = recentDistinctEntries(
        batch.entries.filter((entry) => !sameStream || entry.sequence > watermark),
        batch.streamId,
        maxEntries,
      );
      const incoming = recentIncoming.entries;
      if (incoming.length === 0) return;

      const sequenceGap = hasSequenceGap(recentIncoming.allSequences, watermark + 1);
      streamId = batch.streamId;
      entries = sameStream ? [...entries, ...incoming] : incoming;
      latestSequence = incoming[incoming.length - 1]?.sequence ?? (sameStream ? latestSequence : null);
      truncated = truncated || !sameStream || sequenceGap || recentIncoming.truncated;
      if (entries.length > maxEntries) {
        entries = entries.slice(entries.length - maxEntries);
        truncated = true;
      }
      publish();
    },
    clear: () => {
      if (entries.length === 0) return;
      entries = [];
      truncated = false;
      publish();
    },
  };
}

export const logBuffer = createDiagnosticLogBuffer();
