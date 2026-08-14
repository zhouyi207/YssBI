import type { ResultDescriptor } from '../types';
import { resolveResultRenderer } from '../resolveRenderer';
import { ReportResultView } from './ReportResultView';
import {
  JsonResultView,
  ScalarResultView,
  DataSeriesResultView,
  SequenceResultView,
} from './renderers/ResultRenderers';

export interface UnifiedResultViewProps {
  payload: ResultDescriptor;
}

export function UnifiedResultView({ payload }: UnifiedResultViewProps) {
  const kind = resolveResultRenderer(payload);

  switch (kind) {
    case 'sequence':
      return <SequenceResultView payload={payload} />;
    case 'dataseries':
      return <DataSeriesResultView payload={payload} />;
    case 'scalar':
      return <ScalarResultView payload={payload} />;

    case 'json':
      return <JsonResultView payload={payload} />;
    case 'info':
      return <ReportResultView payload={payload} />;
    default:
      return <JsonResultView payload={payload} />;
  }
}
