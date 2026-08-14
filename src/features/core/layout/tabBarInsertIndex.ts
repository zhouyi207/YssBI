export interface TabBarTabMetric {
  tabId: string;
  index: number;
  left: number;
  width: number;
}

const DEFAULT_GAP_WIDTH = 80;

/** Insert index from pointer X — VS Code uses whole-strip hit testing, not per-tab hover. */
export function computeTabInsertIndex(
  pointerX: number,
  metrics: readonly TabBarTabMetric[],
): number {
  for (const metric of metrics) {
    const midpoint = metric.left + metric.width / 2;
    if (pointerX < midpoint) return metric.index;
  }
  return metrics.length > 0 ? metrics[metrics.length - 1].index + 1 : 0;
}

/** Read tab metrics from a tab-strip container (`[data-tab-id]` children). */
export function measureTabBarMetrics(
  stripElement: HTMLElement,
  tabIds: readonly string[],
): TabBarTabMetric[] {
  const viewport = stripElement.closest('[data-slot="scroll-area-viewport"]') as HTMLElement | null;
  const scrollLeft = viewport?.scrollLeft ?? stripElement.scrollLeft;
  const stripRect = stripElement.getBoundingClientRect();
  return tabIds.flatMap((tabId, index) => {
    const element = stripElement.querySelector(`[data-tab-id="${tabId}"]`) as HTMLElement | null;
    if (!element) return [];
    const rect = element.getBoundingClientRect();
    return [{
      tabId,
      index,
      left: rect.left - stripRect.left + scrollLeft,
      width: rect.width,
    }];
  });
}

/** Pixel offset of the preview gap inside the tab strip. */
export function computeTabGapLeft(
  metrics: readonly TabBarTabMetric[],
  insertIndex: number,
  draggedTabId: string | null,
): number {
  if (insertIndex <= 0) return 0;

  let left = 0;
  for (const metric of metrics) {
    if (metric.index >= insertIndex) break;
    if (metric.tabId === draggedTabId) continue;
    left = metric.left + metric.width;
  }
  return left;
}

export function resolveTabGapWidth(
  metrics: readonly TabBarTabMetric[],
  draggedTabId: string | null,
): number {
  if (!draggedTabId) return DEFAULT_GAP_WIDTH;
  const dragged = metrics.find((metric) => metric.tabId === draggedTabId);
  return dragged?.width ?? DEFAULT_GAP_WIDTH;
}

/**
 * Shift amount for non-dragged tabs to open a gap at `insertIndex`.
 * Mirrors VS Code tab-strip compression while dragging within the same group.
 */
export function computeTabShiftOffset(
  tabIndex: number,
  draggedIndex: number,
  insertIndex: number,
  gapWidth: number,
): number {
  if (draggedIndex < 0) {
    return tabIndex >= insertIndex ? gapWidth : 0;
  }
  if (tabIndex === draggedIndex) return 0;

  if (draggedIndex < insertIndex) {
    if (tabIndex > draggedIndex && tabIndex < insertIndex) return -gapWidth;
    return 0;
  }

  if (draggedIndex > insertIndex) {
    if (tabIndex >= insertIndex && tabIndex < draggedIndex) return gapWidth;
    return 0;
  }

  return 0;
}
