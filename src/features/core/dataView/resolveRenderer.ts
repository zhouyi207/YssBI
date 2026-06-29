import type { DataViewPayload, DataViewRendererKind } from './types';

export function resolveDataViewRenderer(payload: DataViewPayload): DataViewRendererKind {
  switch (payload.dataType) {
    case 'dataframe':
      return 'dataframe';
    case 'series':
      return 'series';
    case 'null':
      return 'null';
    case 'scalar':
      return 'scalar';
    case 'struct':
      return payload.structKind === 'ols_result' ? 'struct_ols' : 'struct_generic';
    default:
      return 'struct_generic';
  }
}
