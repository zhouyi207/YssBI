import { describe, expect, it } from 'vitest';
import type { ResultDescriptor } from '@/shared/types/domain/result';
import { resolveResultRenderer } from './resolveRenderer';

function descriptor(valueKind: ResultDescriptor['valueKind']): ResultDescriptor {
  return {
    resultId: '17',
    state: { kind: 'ready' },
    provenance: {
      runId: '1', activationId: '2', graphPath: 'events/Main.yssbi-event', graphRevision: '3',
      nodeId: '00000000-0000-0000-0000-000000000002', output: null, createdAtMs: '4',
    },
    presentation: { kind: 'inspector' },
    valueKind,
    metadata: null,
    totalCount: 1,
    title: 'Result',
  };
}

describe('resolveResultRenderer', () => {
  it('selects renderers from result value kind', () => {
    expect(resolveResultRenderer(descriptor('sequence'))).toBe('sequence');
    expect(resolveResultRenderer(descriptor('dataSeries'))).toBe('dataseries');
    expect(resolveResultRenderer(descriptor('scalar'))).toBe('scalar');
    expect(resolveResultRenderer(descriptor('unknown'))).toBe('json');
  });

  it('selects plot and report from presentation', () => {
    expect(resolveResultRenderer({
      ...descriptor('scalar'), presentation: { kind: 'plot', chart: 'scatter' },
    })).toBe('plot');
    expect(resolveResultRenderer({
      ...descriptor('scalar'), presentation: { kind: 'report', report: 'olsSummary' },
    })).toBe('info');
  });
});
