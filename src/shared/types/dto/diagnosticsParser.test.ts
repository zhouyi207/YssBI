import { describe, expect, it } from 'vitest';
import {
  parseDiagnosticBatchDto,
  parseDiagnosticRecordDto,
  parseDiagnosticSubscriptionDto,
} from './diagnosticsParser';

const record = {
  streamId: 'diagnostics-1',
  sequence: 7,
  timestamp: '2026-08-16T10:11:12.000Z',
  level: 'warn',
  origin: 'frontend',
  domain: 'graph',
  target: 'GraphManagement',
  event: 'graph.open.failed',
  message: 'Could not open graph',
  source: 'openGraphInEditor',
  fields: {
    graphPath: 'events/Main.yssbi-event',
    retryable: false,
    attempts: 2,
    nested: { reason: null },
  },
};

describe('diagnostic DTO parsers', () => {
  it('accepts strict camelCase records, subscriptions, and batches', () => {
    expect(parseDiagnosticRecordDto(record)).toEqual(record);
    expect(parseDiagnosticSubscriptionDto({
      subscriptionId: 'subscription-1',
      streamId: 'diagnostics-1',
      entries: [record],
      latestSequence: 7,
      truncated: false,
    })).toMatchObject({ subscriptionId: 'subscription-1', entries: [record] });
    expect(parseDiagnosticBatchDto({
      streamId: 'diagnostics-1',
      entries: [record],
    })).toEqual({ streamId: 'diagnostics-1', entries: [record] });
  });

  it('accepts omitted optional event and source fields', () => {
    const { event: _event, source: _source, ...required } = record;
    expect(parseDiagnosticRecordDto(required)).toEqual(required);
  });

  it('rejects aliases, foreign keys, invalid levels, and non-JSON fields', () => {
    const { streamId: _streamId, ...withoutStreamId } = record;
    expect(() => parseDiagnosticRecordDto({ ...withoutStreamId, stream_id: 'diagnostics-1' })).toThrow();
    expect(() => parseDiagnosticRecordDto({ ...record, logType: 'graph' })).toThrow();
    expect(() => parseDiagnosticRecordDto({ ...record, level: 'warning' })).toThrow();
    expect(() => parseDiagnosticRecordDto({ ...record, origin: 'backend' })).toThrow();
    expect(() => parseDiagnosticRecordDto({ ...record, domain: 'notify' })).toThrow();
    expect(() => parseDiagnosticRecordDto({
      ...record,
      fields: { duration: Number.POSITIVE_INFINITY },
    })).toThrow();
  });

  it('rejects records from another stream and impossible subscription watermarks', () => {
    expect(() => parseDiagnosticBatchDto({
      streamId: 'diagnostics-2',
      entries: [record],
    })).toThrow();
    expect(() => parseDiagnosticSubscriptionDto({
      subscriptionId: 'subscription-1',
      streamId: 'diagnostics-1',
      entries: [record],
      latestSequence: 6,
      truncated: false,
    })).toThrow();
  });
});
