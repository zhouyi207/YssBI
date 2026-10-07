import { createStore } from "zustand/vanilla";
import { shallow } from "zustand/shallow";
import { LOG_BUFFER_MAX } from "@/shared/config-default";
import {
  LOG_DOMAINS,
  type LogDomain,
  type LogBatchDto,
  type LogRecordDto,
  type LogSubscriptionDto,
} from "@/shared/types/domain/log";
import type { LogDomainId } from "@/features/domain/log/logDomains";

type LogDomainEntries = Readonly<Record<LogDomainId, readonly LogRecordDto[]>>;

export interface LogSnapshot {
  readonly streamId: string | null;
  readonly entries: readonly LogRecordDto[];
  readonly entriesByDomain: LogDomainEntries;
  readonly latestSequence: number | null;
  readonly truncated: boolean;
}

export interface LogLogBuffer {
  subscribe: (listener: () => void) => () => void;
  getSnapshot: () => LogSnapshot;
  setSubscription: (subscription: LogSubscriptionDto) => void;
  appendBatch: (batch: LogBatchDto) => void;
  markTruncated: () => void;
  clear: () => void;
}

interface RecentEntries {
  entries: LogRecordDto[];
  allSequences: number[];
  truncated: boolean;
}

function indexEntries(
  entries: readonly LogRecordDto[],
  previous?: LogDomainEntries,
): LogDomainEntries {
  const domains = Object.fromEntries<LogRecordDto[]>(
    LOG_DOMAINS.map((domain) => [domain, []]),
  ) as Record<LogDomain, LogRecordDto[]>;
  for (const entry of entries) domains[entry.domain].push(entry);
  const index: Record<LogDomainId, readonly LogRecordDto[]> = { all: entries, ...domains };
  for (const domain of LOG_DOMAINS) {
    if (previous && shallow(previous[domain], index[domain])) index[domain] = previous[domain];
  }
  return index;
}

function appendDomainEntries(
  previous: LogSnapshot,
  entries: readonly LogRecordDto[],
  incoming: readonly LogRecordDto[],
): LogDomainEntries {
  const removed = new Map<LogDomain, number>();
  const removedCount = previous.entries.length + incoming.length - entries.length;
  for (let index = 0; index < removedCount; index += 1) {
    const domain = previous.entries[index].domain;
    removed.set(domain, (removed.get(domain) ?? 0) + 1);
  }
  const added = new Map<LogDomain, LogRecordDto[]>();
  for (const entry of incoming) {
    let domainEntries = added.get(entry.domain);
    if (!domainEntries) added.set(entry.domain, (domainEntries = []));
    domainEntries.push(entry);
  }
  const index = { ...previous.entriesByDomain, all: entries };
  for (const domain of LOG_DOMAINS) {
    const removeCount = removed.get(domain) ?? 0;
    const incomingDomain = added.get(domain);
    if (!removeCount && !incomingDomain) continue;
    const retained = removeCount
      ? previous.entriesByDomain[domain].slice(removeCount)
      : previous.entriesByDomain[domain];
    index[domain] = incomingDomain ? [...retained, ...incomingDomain] : retained;
  }
  return index;
}

function recentDistinctEntries(
  entries: readonly LogRecordDto[],
  streamId: string,
  maxEntries: number,
): RecentEntries {
  const bySequence = new Map<number, LogRecordDto>();
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

export function createLogLogBuffer(maxEntries = LOG_BUFFER_MAX): LogLogBuffer {
  if (!Number.isInteger(maxEntries) || maxEntries <= 0) {
    throw new Error("Log log buffer capacity must be a positive integer");
  }

  const store = createStore<LogSnapshot>(() => {
    const entries: LogRecordDto[] = [];
    return {
      streamId: null,
      entries,
      entriesByDomain: indexEntries(entries),
      latestSequence: null,
      truncated: false,
    };
  });
  let replacedStream = false;

  return {
    subscribe: store.subscribe,
    getSnapshot: store.getState,
    setSubscription: (subscription) => {
      const snapshot = store.getState();
      const { streamId } = snapshot;
      replacedStream ||= streamId !== null && streamId !== subscription.streamId;
      const recent = recentDistinctEntries(subscription.entries, subscription.streamId, maxEntries);
      const snapshotGap = hasSequenceGap(
        recent.allSequences,
        subscription.truncated ? (recent.allSequences[0] ?? subscription.latestSequence + 1) : 1,
        subscription.latestSequence,
      );
      // Persisted records are append-only. A reconnect can reuse records still held
      // by this buffer, but sequence numbers from another stream have no relation.
      const previousEntries =
        streamId === subscription.streamId
          ? new Map(snapshot.entries.map((entry) => [entry.sequence, entry]))
          : undefined;
      const shared = previousEntries
        ? recent.entries.map((entry) => previousEntries.get(entry.sequence) ?? entry)
        : recent.entries;
      const entries = shallow(snapshot.entries, shared) ? snapshot.entries : shared;
      const next = {
        streamId: subscription.streamId,
        entries,
        entriesByDomain:
          entries === snapshot.entries
            ? snapshot.entriesByDomain
            : indexEntries(entries, snapshot.entriesByDomain),
        latestSequence: subscription.latestSequence,
        truncated: replacedStream || subscription.truncated || recent.truncated || snapshotGap,
      };
      if (!shallow(snapshot, next)) store.setState(next);
    },
    appendBatch: (batch) => {
      const snapshot = store.getState();
      const { streamId, latestSequence } = snapshot;
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
      let entries = sameStream ? [...snapshot.entries, ...incoming] : incoming;
      let truncated = snapshot.truncated || !sameStream || sequenceGap || recentIncoming.truncated;
      if (entries.length > maxEntries) {
        entries = entries.slice(entries.length - maxEntries);
        truncated = true;
      }
      store.setState({
        streamId: batch.streamId,
        entries,
        entriesByDomain: sameStream
          ? appendDomainEntries(snapshot, entries, incoming)
          : indexEntries(entries, snapshot.entriesByDomain),
        latestSequence:
          incoming[incoming.length - 1]?.sequence ?? (sameStream ? latestSequence : null),
        truncated,
      });
    },
    markTruncated: () => {
      if (!store.getState().truncated) store.setState({ truncated: true });
    },
    clear: () => {
      const snapshot = store.getState();
      if (snapshot.entries.length === 0 && !snapshot.truncated) return;
      replacedStream = false;
      const entries = snapshot.entries.length ? [] : snapshot.entries;
      store.setState({
        entries,
        entriesByDomain:
          entries === snapshot.entries
            ? snapshot.entriesByDomain
            : indexEntries(entries, snapshot.entriesByDomain),
        truncated: false,
      });
    },
  };
}

export const logBuffer = createLogLogBuffer();
