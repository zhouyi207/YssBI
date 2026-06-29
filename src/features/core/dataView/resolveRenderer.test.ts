import { describe, expect, it } from 'vitest';
import { resolveDataViewRenderer } from './resolveRenderer';
import type { SourceDescriptor } from './types';

function descriptor(partial: Partial<SourceDescriptor> & Pick<SourceDescriptor, 'renderer' | 'title'>): SourceDescriptor {
  return {
    sourceId: 'source-1',
    kind: 'struct',
    ...partial,
  };
}

describe('resolveDataViewRenderer', () => {
  it('selects dataframe renderer from metadata only', () => {
    expect(
      resolveDataViewRenderer(
        descriptor({ kind: 'dataframe', renderer: 'dataframe', title: 'DF', totalRows: 1000 }),
      ),
    ).toBe('dataframe');
  });

  it('selects series renderer from metadata only', () => {
    expect(
      resolveDataViewRenderer(
        descriptor({ kind: 'series', renderer: 'series', title: 'S', length: 500 }),
      ),
    ).toBe('series');
  });

  it('selects scalar and null renderers', () => {
    expect(resolveDataViewRenderer(descriptor({ kind: 'scalar', renderer: 'scalar', title: 'X' }))).toBe('scalar');
    expect(resolveDataViewRenderer(descriptor({ kind: 'null', renderer: 'null', title: 'Empty' }))).toBe('null');
  });

  it('selects OLS struct renderer from metadata struct kind', () => {
    expect(
      resolveDataViewRenderer(
        descriptor({
          kind: 'struct',
          renderer: 'struct_ols',
          title: 'OLS',
          structKind: 'ols_result',
        }),
      ),
    ).toBe('struct_ols');
  });

  it('falls back to generic struct renderer', () => {
    expect(
      resolveDataViewRenderer(
        descriptor({ kind: 'struct', renderer: 'struct_generic', title: 'Unknown', typeKey: 'Foo' }),
      ),
    ).toBe('struct_generic');
  });
});
