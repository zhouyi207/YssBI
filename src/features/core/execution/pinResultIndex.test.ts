import { describe, expect, it } from 'vitest';
import type { PinResultState } from '@/shared/types/ui';
import {
  executionStatusForSourceGraph,
  lookupPinResult,
  pinResultCacheKey,
  pinResultsForSourceGraph,
} from './pinResultIndex';
import type { GraphExecutionState } from '@/shared/types/ui';

function sampleResult(graphPath: string, pinId: string): PinResultState {
  return {
    graphPath,
    nodeId: 'node-1',
    pinId,
    sourceId: `source-${pinId}`,
    descriptor: {
      sourceId: `source-${pinId}`,
      kind: 'json',
      presentation: { kind: 'inspector' },
      title: 'Result',
    },
  };
}

function bucket(
  status: GraphExecutionState['status'],
  results: PinResultState[],
): GraphExecutionState {
  const pinResults = new Map(
    results.map((result) => [pinResultCacheKey(result.graphPath, result.pinId), result]),
  );
  return {
    status,
    runId: null,
    nodeStates: new Map(),
    completedConnections: new Set(),
    flowingConnections: new Set(),
    recording: [],
    graphDirty: false,
    pinResults,
  };
}

describe('pinResultIndex', () => {
  it('uses composite cache keys', () => {
    expect(pinResultCacheKey('events/Main.yssbi-event', 'out-1')).toBe(
      'events/Main.yssbi-event:out-1',
    );
  });

  it('merges nested function results for source graph views', () => {
    const graphs = {
      'events/Main.yssbi-event': bucket('completed', [
        sampleResult('functions/Helper.yssbi-function', 'out-fn'),
      ]),
    };

    const merged = pinResultsForSourceGraph(graphs, 'functions/Helper.yssbi-function');
    expect(merged.size).toBe(1);
    expect(merged.get('functions/Helper.yssbi-function:out-fn')?.pinId).toBe('out-fn');
    expect(executionStatusForSourceGraph(graphs, 'functions/Helper.yssbi-function')).toBe(
      'completed',
    );
  });

  it('lookupPinResult falls back to pinId when graphPath differs (nested call)', () => {
    const pinResults = new Map([
      [
        pinResultCacheKey('functions/Helper.yssbi-function', 'out-1'),
        sampleResult('functions/Helper.yssbi-function', 'out-1'),
      ],
    ]);

    const hit = lookupPinResult(pinResults, 'events/Main.yssbi-event', 'out-1');
    expect(hit?.graphPath).toBe('functions/Helper.yssbi-function');
  });
});
