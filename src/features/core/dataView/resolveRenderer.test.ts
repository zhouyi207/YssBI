import { describe, expect, it } from 'vitest';
import { resolveDataViewRenderer } from './resolveRenderer';
import type { DataViewPayload } from './types';

function payload(partial: Partial<DataViewPayload> & Pick<DataViewPayload, 'dataType' | 'title'>): DataViewPayload {
  return {
    sourceId: 'source-1',
    windowType: 'data_view',
    viewType: 'data_view',
    ...partial,
  };
}

describe('resolveDataViewRenderer', () => {
  it('selects dataframe renderer from metadata only', () => {
    expect(
      resolveDataViewRenderer(
        payload({ dataType: 'dataframe', title: 'DF', totalRows: 1000 }),
      ),
    ).toBe('dataframe');
  });

  it('selects series renderer from metadata only', () => {
    expect(
      resolveDataViewRenderer(
        payload({ dataType: 'series', title: 'S', length: 500 }),
      ),
    ).toBe('series');
  });

  it('selects scalar and null renderers', () => {
    expect(resolveDataViewRenderer(payload({ dataType: 'scalar', title: 'X' }))).toBe('scalar');
    expect(resolveDataViewRenderer(payload({ dataType: 'null', title: 'Empty' }))).toBe('null');
  });

  it('selects OLS struct renderer from metadata struct kind', () => {
    expect(
      resolveDataViewRenderer(
        payload({
          dataType: 'struct',
          title: 'OLS',
          structKind: 'ols_result',
        }),
      ),
    ).toBe('struct_ols');
  });

  it('falls back to generic struct renderer', () => {
    expect(
      resolveDataViewRenderer(
        payload({ dataType: 'struct', title: 'Unknown', typeKey: 'Foo' }),
      ),
    ).toBe('struct_generic');
  });
});
