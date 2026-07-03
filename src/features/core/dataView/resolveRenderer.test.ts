import { describe, expect, it } from 'vitest';
import { resolveDataViewRenderer } from './resolveRenderer';
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

describe('resolveDataViewRenderer', () => {
  it('selects dataframe renderer from kind when inspector', () => {
    expect(
      resolveDataViewRenderer(
        descriptor({ kind: 'dataframe', title: 'DF', totalRows: 1000 }),
      ),
    ).toBe('dataframe');
  });

  it('selects series renderer from kind when inspector', () => {
    expect(
      resolveDataViewRenderer(
        descriptor({ kind: 'dataseries', title: 'S', length: 500 }),
      ),
    ).toBe('dataseries');
  });

  it('selects scalar, null, and json renderers', () => {
    expect(resolveDataViewRenderer(descriptor({ kind: 'scalar', title: 'X' }))).toBe('scalar');
    expect(resolveDataViewRenderer(descriptor({ kind: 'null', title: 'Empty' }))).toBe('null');
    expect(resolveDataViewRenderer(descriptor({ kind: 'json', title: 'Object' }))).toBe('json');
  });

  it('selects plot and report renderers from presentation', () => {
    expect(
      resolveDataViewRenderer({
        ...descriptor({ kind: 'json', title: 'Plot' }),
        presentation: { kind: 'plot', chart: 'scatter' },
      }),
    ).toBe('plot');
    expect(
      resolveDataViewRenderer({
        ...descriptor({ kind: 'json', title: 'Report' }),
        presentation: { kind: 'report', report: 'olsSummary' },
      }),
    ).toBe('info');
  });
});
