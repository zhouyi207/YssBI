import type { DetailTarget, DetailTargetInput } from '@/shared/types/ui/detail';

export function resolveDetailTarget(input: DetailTargetInput): DetailTarget | null {
  const { detailFocus, selectedLog } = input;
  if (!detailFocus) return null;
  if (detailFocus.kind === 'log' && !selectedLog) return null;
  return detailFocus;
}
