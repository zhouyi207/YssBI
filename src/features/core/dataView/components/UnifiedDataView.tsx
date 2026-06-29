import type { DataViewPayload } from '../types';
import { resolveDataViewRenderer } from '../resolveRenderer';
import {
  DataFrameSourceView,
  GenericStructSourceView,
  NullSourceView,
  OlsStructSourceView,
  ScalarSourceView,
  SeriesSourceView,
} from './renderers/DataViewSourceRenderers';

export interface UnifiedDataViewProps {
  payload: DataViewPayload;
}

export function UnifiedDataView({ payload }: UnifiedDataViewProps) {
  const kind = resolveDataViewRenderer(payload);

  switch (kind) {
    case 'dataframe':
      return <DataFrameSourceView payload={payload} />;
    case 'series':
      return <SeriesSourceView payload={payload} />;
    case 'scalar':
      return <ScalarSourceView payload={payload} />;
    case 'null':
      return <NullSourceView payload={payload} />;
    case 'struct_ols':
      return <OlsStructSourceView payload={payload} />;
    case 'struct_generic':
    default:
      return <GenericStructSourceView payload={payload} />;
  }
}
