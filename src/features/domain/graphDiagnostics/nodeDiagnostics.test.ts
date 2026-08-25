import { describe, expect, it } from 'vitest';
import type { DiagnosticDto } from '@/shared/types/dto/editorProjection';
import {
  collectNodeDiagnostics,
  type GraphNodeDiagnosticsBucket,
} from './nodeDiagnostics';

const diagnostic = (code: string, message: string): DiagnosticDto => ({
  code,
  message,
  severity: code === 'error' ? 'error' : 'warning',
  blocking: code === 'error',
  location: { kind: 'node', nodeId: 'unused-in-fixture' },
  related: [],
});

const bucket = {
  graphNodes: ['node-a', 'node-b'],
  nodes: {
    'node-a': {
      id: 'node-a',
      title: 'Raw A',
      display: { title: 'Node A' },
      diagnostics: [diagnostic('error', 'A failed'), diagnostic('warning', 'A needs review')],
    },
    'node-b': {
      id: 'node-b',
      title: 'Raw B',
      diagnostics: [diagnostic('warning', 'B needs review')],
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
        diagnostic: diagnostic('error', 'A failed'),
      },
      {
        graphPath: 'events/Main.yssbi-event',
        nodeId: 'node-a',
        nodeTitle: 'Node A',
        diagnostic: diagnostic('warning', 'A needs review'),
      },
      {
        graphPath: 'events/Main.yssbi-event',
        nodeId: 'node-b',
        nodeTitle: 'Raw B',
        diagnostic: diagnostic('warning', 'B needs review'),
      },
    ]);
  });

  it('returns no rows when the graph projection is unavailable', () => {
    expect(collectNodeDiagnostics('events/Main.yssbi-event', undefined)).toEqual([]);
  });
});
