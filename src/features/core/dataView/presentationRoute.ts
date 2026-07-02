import type { SourceDescriptor } from '@/features/core/dataView/types';

/** Resolve child window hash route from backend source descriptor metadata. */
export function presentationRouteForDescriptor(descriptor: SourceDescriptor): string {
  switch (descriptor.renderer) {
    case 'plot':
      return '/plot';
    case 'dataframe':
    case 'dataseries':
    case 'scalar':
    case 'null':
    case 'json':
      return '/view';
    default:
      return '/info';
  }
}
