import type { ResultDescriptor, ResultRendererKind } from './types';

export function resolveResultRenderer(descriptor: ResultDescriptor): ResultRendererKind {
  if (descriptor.presentation.kind === 'plot') return 'plot';
  if (descriptor.presentation.kind === 'report') return 'info';
  switch (descriptor.valueKind) {
    case 'sequence':
      return 'sequence';
    case 'dataSeries':
      return 'dataseries';
    case 'scalar':
      return 'scalar';
    case 'unknown':
      return 'json';
  }
}
