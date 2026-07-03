import type { SourceDescriptor } from '../types';
import type { SourceViewLayout } from './SourceViewShell';
import { resolveSourceRenderer } from '../resolveRenderer';
import {
  DataFrameSourceView,
  JsonSourceView,
  NullSourceView,
  ScalarSourceView,
  DataSeriesSourceView,
} from './renderers/SourceRenderers';

export interface UnifiedSourceViewProps {
  payload: SourceDescriptor;
  layout?: SourceViewLayout;
}

export function UnifiedSourceView({ payload, layout = 'embedded' }: UnifiedSourceViewProps) {
  const kind = resolveSourceRenderer(payload);
  const viewProps = { payload, layout };

  switch (kind) {
    case 'dataframe':
      return <DataFrameSourceView {...viewProps} />;
    case 'dataseries':
      return <DataSeriesSourceView {...viewProps} />;
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
