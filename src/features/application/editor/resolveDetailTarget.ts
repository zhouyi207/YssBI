import type { DetailTarget, DetailTargetInput } from "@/features/core/editor/detail/detailTypes";

export function resolveDetailTarget(input: DetailTargetInput): DetailTarget | null {
  const { detailFocus, selectedLog } = input;
  if (!detailFocus) return null;
  if (detailFocus.kind === "log" && !selectedLog) return null;
  return detailFocus;
}
