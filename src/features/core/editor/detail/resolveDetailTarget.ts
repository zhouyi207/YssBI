import type { DetailTarget, DetailTargetInput } from './types';

export function resolveDetailTarget(input: DetailTargetInput): DetailTarget | null {
  const { detailFocus, selectedLog } = input;
  if (!detailFocus) return null;
  if (detailFocus.kind === 'log' && !selectedLog) return null;
  return detailFocus;
}
