import type { LayoutNode } from '@/shared/types/ui';

export type FlexSplitPair = {
  beforeId: string;
  afterId: string;
  beforeStart: number;
  afterStart: number;
};

/** True when neither sibling has a fixed pixelSize — first sash drag should pixelize both. */
export function isFlexSplitPair(beforeNode: LayoutNode | undefined, afterNode: LayoutNode | undefined): boolean {
  if (!beforeNode || !afterNode) return false;
  if (beforeNode.data?.visible === false || afterNode.data?.visible === false) return false;
  return beforeNode.pixelSize == null && afterNode.pixelSize == null;
}

export function computeFlexSplitSizes(
  pair: FlexSplitPair,
  pointerDelta: number,
  minBefore = 0,
  minAfter = 0,
): { beforeSize: number; afterSize: number } {
  const total = pair.beforeStart + pair.afterStart;
  const beforeSize = Math.min(
    total - minAfter,
    Math.max(minBefore, pair.beforeStart + pointerDelta),
  );
  return {
    beforeSize,
    afterSize: total - beforeSize,
  };
}
