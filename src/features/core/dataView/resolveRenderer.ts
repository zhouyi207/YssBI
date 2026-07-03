import type { SourceDescriptor, DataViewRendererKind } from './types';

export function resolveDataViewRenderer(descriptor: SourceDescriptor): DataViewRendererKind {
  const { presentation, kind } = descriptor;
  if (presentation.kind === 'plot') return 'plot';
  if (presentation.kind === 'report') return 'info';

  switch (kind) {
    case 'dataframe':
      return 'dataframe';
    case 'dataseries':
      return 'dataseries';
    case 'scalar':
      return 'scalar';
    case 'null':
      return 'null';
    case 'json':
    case 'struct':
    default:
      return 'json';
  }
}
