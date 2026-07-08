import { describe, expect, it } from 'vitest';
import type { PinResultState } from '@/shared/types/ui';
import {
  buildPinResultSearchEntry,
  collectPinResultSearchEntries,
  filterPinResultSearchEntries,
} from './pinResultSearch';

function pinResult(pinId: string, title: string, nodeId = 'node-1'): PinResultState {
  return {
    graphPath: 'graph-1',
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
  it('builds searchable entries with node and pin labels', () => {
    const entries = [
      buildPinResultSearchEntry(
        'output:out-1',
        'output',
        pinResult('out-1', 'OLS Result'),
        { nodeTitle: 'OLS Regression', pinName: 'Result' },
      ),
    ];

    expect(entries).toHaveLength(1);
    expect(entries[0]?.nodeTitle).toBe('OLS Regression');
    expect(entries[0]?.pinName).toBe('Result');
    expect(entries[0]?.sourceTitle).toBe('OLS Result');
    expect(entries[0]?.direction).toBe('output');
  });

  it('filters entries by node, pin, or source title', () => {
    const entries = [
      buildPinResultSearchEntry(
        'output:out-1',
        'output',
        pinResult('out-1', 'Alpha Table', 'node-a'),
        { nodeTitle: 'Alpha Node', pinName: 'Output' },
      ),
      buildPinResultSearchEntry(
        'output:out-2',
        'output',
        pinResult('out-2', 'Beta Table', 'node-b'),
        { nodeTitle: 'Beta Node', pinName: 'Output' },
      ),
    ];

    expect(filterPinResultSearchEntries(entries, 'alpha')).toHaveLength(1);
    expect(filterPinResultSearchEntries(entries, 'output')).toHaveLength(2);
    expect(filterPinResultSearchEntries(entries, 'beta table')).toHaveLength(1);
  });

  it('includes connected input pins that resolve to upstream results', () => {
    const pinResults = new Map([['out-1', pinResult('out-1', 'Upstream Table', 'node-out')]]);
    const entries = collectPinResultSearchEntries(
      'graph-1',
      pinResults,
      [
        {
          pinId: 'out-1',
          nodeId: 'node-out',
          direction: 'output',
          isExec: false,
          connectionIds: ['out-1->in-1'],
        },
        {
          pinId: 'in-1',
          nodeId: 'node-in',
          direction: 'input',
          isExec: false,
          connectionIds: ['out-1->in-1'],
        },
      ],
      (_nodeId, pinId) =>
        pinId === 'in-1'
          ? { nodeTitle: 'Summary', pinName: 'Data' }
          : { nodeTitle: 'OLS', pinName: 'Result' },
    );

    expect(entries).toHaveLength(2);
    expect(entries[0]?.direction).toBe('output');
    expect(entries[1]?.direction).toBe('input');
    expect(entries[1]?.nodeTitle).toBe('Summary');
    expect(entries[1]?.pinName).toBe('Data');
    expect(entries[1]?.pinResult.pinId).toBe('out-1');
  });
});
