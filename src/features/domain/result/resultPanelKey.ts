import { graphOutputKey } from '@/features/domain/editorProjection';
import type { ResultDescriptor } from '@/shared/types/dto/result';

export function resultPanelKey(descriptor: ResultDescriptor): string {
  const output = descriptor.provenance.output;
  return output
    ? `output:${graphOutputKey(output)}`
    : `result:${descriptor.resultId.length}:${descriptor.resultId}`;
}
