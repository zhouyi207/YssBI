import type { SourceDescriptor, DataViewRendererKind } from './types';

export function resolveDataViewRenderer(descriptor: SourceDescriptor): DataViewRendererKind {
  return descriptor.renderer;
}
