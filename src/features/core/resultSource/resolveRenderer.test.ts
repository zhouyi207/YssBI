import { describe, expect, it } from 'vitest';
import { resolveSourceRenderer } from './resolveRenderer';
import type { SourceDescriptor } from './types';

function descriptor(
  partial: Partial<SourceDescriptor> & Pick<SourceDescriptor, 'kind' | 'title'>,
): SourceDescriptor {
  return {
    sourceId: 'source-1',
    presentation: { kind: 'inspector' },
    ...partial,
  };
}

describe('resolveSourceRenderer', () => {
  it('selects dataframe renderer from kind when inspector', () => {
    expect(
      resolveSourceRenderer(
        descriptor({ kind: 'dataframe', title: 'DF', totalRows: 1000 }),
      ),
    ).toBe('dataframe');
  });

  it('selects series renderer from kind when inspector', () => {
    expect(
      resolveSourceRenderer(
        descriptor({ kind: 'dataseries', title: 'S', length: 500 }),
      ),
    ).toBe('dataseries');
  });

  it('selects scalar, null, and json renderers', () => {
    expect(resolveSourceRenderer(descriptor({ kind: 'scalar', title: 'X' }))).toBe('scalar');
    expect(resolveSourceRenderer(descriptor({ kind: 'null', title: 'Empty' }))).toBe('null');
    expect(resolveSourceRenderer(descriptor({ kind: 'json', title: 'Object' }))).toBe('json');
  });

  it('selects plot and report renderers from presentation', () => {
    expect(
      resolveSourceRenderer({
        ...descriptor({ kind: 'json', title: 'Plot' }),
        presentation: { kind: 'plot', chart: 'scatter' },
      }),
    ).toBe('plot');
    expect(
      resolveSourceRenderer({
        ...descriptor({ kind: 'json', title: 'Report' }),
        presentation: { kind: 'report', report: 'olsSummary' },
      }),
    ).toBe('info');
  });
});
