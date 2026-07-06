import type { SourceDescriptor } from '../types';
import { resolveSourceRenderer } from '../resolveRenderer';
import { ReportSourceView } from './ReportSourceView';
import {
  DataFrameSourceView,
  JsonSourceView,
  NullSourceView,
  ScalarSourceView,
  DataSeriesSourceView,
} from './renderers/SourceRenderers';

export interface UnifiedSourceViewProps {
  payload: SourceDescriptor;
}

export function UnifiedSourceView({ payload }: UnifiedSourceViewProps) {
  const kind = resolveSourceRenderer(payload);

  switch (kind) {
    case 'dataframe':
      return <DataFrameSourceView payload={payload} />;
    case 'dataseries':
      return <DataSeriesSourceView payload={payload} />;
    case 'scalar':
      return <ScalarSourceView payload={payload} />;
    case 'null':
      return <NullSourceView payload={payload} />;
    case 'json':
      return <JsonSourceView payload={payload} />;
    case 'info':
      return <ReportSourceView payload={payload} />;
    default:
      return <JsonSourceView payload={payload} />;
  }
}
