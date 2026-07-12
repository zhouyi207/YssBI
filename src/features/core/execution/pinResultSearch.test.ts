import { describe, expect, it } from 'vitest';
import type { PinResultState } from '@/shared/types/ui';
import { pinResultCacheKey } from './pinResultIndex';
import {
  buildPinResultSearchEntry,
  collectPinResultSearchEntries,
  filterPinResultSearchEntries,
} from './pinResultSearch';

function pinResult(pinId: string, title: string, nodeId = 'node-1'): PinResultState {
  return {
    graphPath: 'events/Main.yssbi-event',
    nodeId,
    pinId,
    sourceId: `source-${pinId}`,
    descriptor: {
      sourceId: `source-${pinId}`,
      kind: 'json',
      presentation: { kind: 'inspector' },
      title,
    },
  };
}

describe('pinResultSearch', () => {
  it('builds searchable entries with runtime pin refs', () => {
    const entry = buildPinResultSearchEntry(pinResult('out-1', 'OLS Result'), {
      nodeTitle: 'OLS Regression',
      pinName: 'Result',
    });

    expect(entry.nodeTitle).toBe('OLS Regression');
    expect(entry.pinName).toBe('Result');
    expect(entry.sourceTitle).toBe('OLS Result');
    expect(entry.id).toBe('events/Main.yssbi-event:out-1');
    expect(entry.ref).toEqual({
      kind: 'runtimePin',
      graphPath: 'events/Main.yssbi-event',
      pinId: 'out-1',
    });
  });

  it('filters entries by node, pin, or source title', () => {
    const entries = [
      buildPinResultSearchEntry(pinResult('out-1', 'Alpha Table', 'node-a'), {
        nodeTitle: 'Alpha Node',
        pinName: 'Output',
      }),
      buildPinResultSearchEntry(pinResult('out-2', 'Beta Table', 'node-b'), {
        nodeTitle: 'Beta Node',
        pinName: 'Output',
      }),
    ];

    expect(filterPinResultSearchEntries(entries, 'alpha')).toHaveLength(1);
    expect(filterPinResultSearchEntries(entries, 'output')).toHaveLength(2);
    expect(filterPinResultSearchEntries(entries, 'beta table')).toHaveLength(1);
  });

  it('collects every cached pin result without walking graph pins', () => {
    const pinResults = new Map([
      [
        pinResultCacheKey('events/Main.yssbi-event', 'out-1'),
        pinResult('out-1', 'Upstream Table', 'node-out'),
      ],
      [
        pinResultCacheKey('events/Main.yssbi-event', 'out-2'),
        pinResult('out-2', 'Second Table', 'node-two'),
      ],
    ]);

    const entries = collectPinResultSearchEntries(
      pinResults,
      (_graphPath, _nodeId, pinId) =>
        pinId === 'out-2'
          ? { nodeTitle: 'Second Node', pinName: 'Data' }
          : { nodeTitle: 'OLS', pinName: 'Result' },
    );

    expect(entries).toHaveLength(2);
    expect(entries.map((entry) => (entry.ref.kind === 'runtimePin' ? entry.ref.pinId : '')).sort()).toEqual(['out-1', 'out-2']);
    expect(
      entries.find((entry) => entry.ref.kind === 'runtimePin' && entry.ref.pinId === 'out-2')?.nodeTitle,
    ).toBe('Second Node');
  });

  it('resolveLabels uses pinResult graphPath for nested call results', () => {
    const pinResults = new Map([
      [
        pinResultCacheKey('functions/Helper.yssbi-function', 'out-fn'),
        {
          ...pinResult('out-fn', 'Fn Result', 'fn-node'),
          graphPath: 'functions/Helper.yssbi-function',
        },
      ],
    ]);

    const entries = collectPinResultSearchEntries(pinResults, (labelGraphPath, nodeId) => ({
      nodeTitle: `${labelGraphPath}:${nodeId}`,
      pinName: 'Result',
    }));

    expect(entries[0]?.nodeTitle).toBe('functions/Helper.yssbi-function:fn-node');
  });
});
