import { describe, expect, it } from 'vitest';
import { portAddressKey } from '@/features/domain/editorProjection';
import type { DiagnosticDto } from '@/shared/types/dto/editorProjection';
import {
  collectNodeDiagnostics,
  type GraphNodeDiagnosticsBucket,
} from './nodeDiagnostics';

const diagnostic = (
  code: string,
  message: string,
  nodeId = 'unused-in-fixture',
): DiagnosticDto => ({
  code,
  message,
  severity: code === 'error' ? 'error' : 'warning',
  blocking: code === 'error',
  location: { kind: 'node', nodeId },
  related: [],
});

const bucket = {
  graphNodes: ['node-a', 'node-b'],
  nodes: {
    'node-a': {
      id: 'node-a',
      title: 'Raw A',
      display: { title: 'Node A' },
      diagnostics: [
        diagnostic('error', 'A failed', 'node-a'),
        diagnostic('warning', 'A needs review', 'node-a'),
      ],
    },
    'node-b': {
      id: 'node-b',
      title: 'Raw B',
      diagnostics: [diagnostic('warning', 'B needs review', 'node-b')],
    },
  },
} as unknown as GraphNodeDiagnosticsBucket;

describe('collectNodeDiagnostics', () => {
  it('flattens every node diagnostic in graph order with projected node titles', () => {
    expect(collectNodeDiagnostics('events/Main.yssbi-event', bucket)).toEqual([
      {
        graphPath: 'events/Main.yssbi-event',
        nodeId: 'node-a',
        nodeTitle: 'Node A',
        locationLabel: 'Node A',
        diagnostic: diagnostic('error', 'A failed', 'node-a'),
      },
      {
        graphPath: 'events/Main.yssbi-event',
        nodeId: 'node-a',
        nodeTitle: 'Node A',
        locationLabel: 'Node A',
        diagnostic: diagnostic('warning', 'A needs review', 'node-a'),
      },
      {
        graphPath: 'events/Main.yssbi-event',
        nodeId: 'node-b',
        nodeTitle: 'Raw B',
        locationLabel: 'Raw B',
        diagnostic: diagnostic('warning', 'B needs review', 'node-b'),
      },
    ]);
  });

  it('returns no rows when the graph projection is unavailable', () => {
    expect(collectNodeDiagnostics('events/Main.yssbi-event', undefined)).toEqual([]);
  });

  it('formats port locations with projected node and pin titles', () => {
    const address = { kind: 'declared' as const, nodeId: 'node-a', portKey: 'value' };
    const portDiagnostic: DiagnosticDto = {
      ...diagnostic('error', 'Value is invalid', 'node-a'),
      location: { kind: 'port', address },
    };
    const pinId = portAddressKey(address);
    const portBucket = {
      graphNodes: ['node-a'],
      nodes: {
        'node-a': {
          id: 'node-a',
          title: 'Raw A',
          display: { title: 'Node A' },
          diagnostics: [portDiagnostic],
        },
      },
      pins: {
        [pinId]: {
          id: pinId,
          nodeId: 'node-a',
          name: 'raw-value',
          display: { label: 'Value', instanceLabel: null },
          address,
        },
      },
    } as unknown as GraphNodeDiagnosticsBucket;

    expect(collectNodeDiagnostics('events/Main.yssbi-event', portBucket)).toEqual([
      {
        graphPath: 'events/Main.yssbi-event',
        nodeId: 'node-a',
        nodeTitle: 'Node A',
        locationLabel: 'Node A · Value',
        diagnostic: portDiagnostic,
      },
    ]);
  });
});
