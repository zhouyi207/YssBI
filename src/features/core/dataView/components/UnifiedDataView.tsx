import type { SourceDescriptor } from '../types';
import type { DataViewLayout } from './components/DataViewShell';
import { resolveDataViewRenderer } from '../resolveRenderer';
import {
  DataFrameSourceView,
  JsonSourceView,
  NullSourceView,
  ScalarSourceView,
  SeriesSourceView,
} from './renderers/DataViewSourceRenderers';

export interface UnifiedDataViewProps {
  payload: SourceDescriptor;
  layout?: DataViewLayout;
}

export function UnifiedDataView({ payload, layout = 'embedded' }: UnifiedDataViewProps) {
  const kind = resolveDataViewRenderer(payload);
  const viewProps = { payload, layout };

  switch (kind) {
    case 'dataframe':
      return <DataFrameSourceView {...viewProps} />;
    case 'series':
      return <SeriesSourceView {...viewProps} />;
    case 'scalar':
      return <ScalarSourceView {...viewProps} />;
    case 'null':
      return <NullSourceView {...viewProps} />;
    case 'json':
      return <JsonSourceView {...viewProps} />;
    default:
      return <JsonSourceView {...viewProps} />;
  }
}
