import { describe, expect, it } from 'vitest';
import type { PinResultState } from '@/shared/types/ui';
import type { PortAddressDto } from '@/shared/types/dto/editorProjection';
import {
  executionStatusForSourceGraph,
  lookupPinPreview,
  lookupPinResult,
  pinPreviewCacheKey,
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
    pinPreviews: new Map(),
  };
}

describe('pinResultIndex', () => {
  it('keys previews by exact stable declared and dynamic addresses', () => {
    const declared: PortAddressDto = {
      kind: 'declared',
      nodeId: 'node-1',
      portKey: 'value',
    };
    const instance: PortAddressDto = {
      kind: 'instance',
      nodeId: 'node-1',
      templateKey: 'value',
      instanceId: 'instance-1',
    };
    const previews = new Map([
      [pinPreviewCacheKey('events/Main.yssbi-event', declared), { port: declared }],
      [pinPreviewCacheKey('events/Main.yssbi-event', instance), { port: instance }],
    ]);

    expect(pinPreviewCacheKey('events/Main.yssbi-event', declared)).not.toBe(
      pinPreviewCacheKey('events/Main.yssbi-event', instance),
    );
    expect(lookupPinPreview(previews, 'events/Main.yssbi-event', instance)?.port).toEqual(instance);
    expect(lookupPinPreview(previews, 'events/Other.yssbi-event', instance)).toBeUndefined();
  });

  it('distinguishes dynamic addresses by instanceId and templateKey independently', () => {
    const base: PortAddressDto = {
      kind: 'instance',
      nodeId: 'node-1',
      templateKey: 'values',
      instanceId: 'instance-1',
    };
    const differentInstance: PortAddressDto = { ...base, instanceId: 'instance-2' };
    const differentTemplate: PortAddressDto = { ...base, templateKey: 'weights' };

    const keys = new Set([
      pinPreviewCacheKey('events/Main.yssbi-event', base),
      pinPreviewCacheKey('events/Main.yssbi-event', differentInstance),
      pinPreviewCacheKey('events/Main.yssbi-event', differentTemplate),
    ]);

    expect(keys.size).toBe(3);
  });

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
