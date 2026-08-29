import { describe, expect, it } from 'vitest';
import type { PinHistoryProjection } from '@/shared/types/ui';
import { pinHistoryCacheKey } from './pinResultIndex';
import {
  buildPinResultSearchEntry,
  collectPinResultSearchEntries,
  filterPinResultSearchEntries,
} from './pinResultSearch';

function history(portKey: string, resultId: string, state: 'ready' | 'cancelled' = 'ready'): PinHistoryProjection {
  const output = { kind: 'declared' as const, nodeId: `node-${portKey}`, portKey };
  return {
    graphPath: 'events/Main.yssbi-event',
    output,
    selectedResultId: resultId,
    entries: [{
      resultId,
      runId: `run-${resultId}`,
      activationId: `activation-${resultId}`,
      graphRevision: '1',
      createdAtMs: '1000',
      usage: { kind: 'produced' },
      state: { kind: state },
    }],
  };
}

describe('pinResultSearch', () => {
  it('builds searchable exact-result entries from history projections', () => {
    const projection = history('result', '17');
    const entry = buildPinResultSearchEntry(projection, {
      nodeTitle: 'OLS Regression',
      pinName: 'Result',
    });

    expect(entry).toMatchObject({
      id: pinHistoryCacheKey(projection.graphPath, projection.output),
      nodeTitle: 'OLS Regression',
      pinName: 'Result',
      sourceTitle: 'ready · 17',
      ref: { kind: 'result', resultId: '17' },
    });
  });

  it('uses selected historical result instead of silently replacing it with latest', () => {
    const projection = history('result', '17');
    projection.entries.push({
      ...projection.entries[0],
      resultId: '18',
      runId: 'run-18',
      state: { kind: 'cancelled' },
    });

    expect(buildPinResultSearchEntry(projection, { nodeTitle: 'Node', pinName: 'Result' })?.ref)
      .toEqual({ kind: 'result', resultId: '17' });
  });

  it('collects and filters cached history projections', () => {
    const first = history('alpha', '17');
    const second = history('beta', '18', 'cancelled');
    const histories = new Map([
      [pinHistoryCacheKey(first.graphPath, first.output), first],
      [pinHistoryCacheKey(second.graphPath, second.output), second],
    ]);
    const entries = collectPinResultSearchEntries(histories, (projection) => ({
      nodeTitle: projection.output.nodeId,
      pinName: projection.output.kind === 'declared' ? projection.output.portKey : projection.output.templateKey,
    }));

    expect(entries).toHaveLength(2);
    expect(filterPinResultSearchEntries(entries, 'cancelled')).toHaveLength(1);
    expect(filterPinResultSearchEntries(entries, 'alpha')).toHaveLength(1);
  });

  it('does not fall back to opaque identities when semantic labels are unavailable', () => {
    const projection = history('result', '17');

    expect(buildPinResultSearchEntry(projection, { nodeTitle: '', pinName: '' })).toMatchObject({
      nodeTitle: '',
      pinName: '',
    });
  });
});
