import type { LayoutNode } from '@/shared/types/ui';

export type FlexSplitPair = {
  beforeId: string;
  afterId: string;
  beforeStart: number;
  afterStart: number;
};

/** True for workbench chrome flex pairs without fixed pixelSize (editor grid uses isEditorGridSash). */
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
  const minimumTotal = minBefore + minAfter;
  if (minimumTotal > total && minimumTotal > 0) {
    const beforeSize = total * (minBefore / minimumTotal);
    return { beforeSize, afterSize: total - beforeSize };
  }
  const beforeSize = Math.min(
    total - minAfter,
    Math.max(minBefore, pair.beforeStart + pointerDelta),
  );
  return {
    beforeSize,
    afterSize: total - beforeSize,
  };
}
