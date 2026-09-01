import { LOG_ITEM_HEIGHT, LOG_ITEM_GAP } from "@/shared/config-default";

export const LOG_ROW_STRIDE = LOG_ITEM_HEIGHT + LOG_ITEM_GAP;

export function estimatedLogListHeight(count: number): number {
  return Math.max(0, count * LOG_ROW_STRIDE);
}

function withInstantScroll(viewport: HTMLElement, update: () => void): void {
  const previousBehavior = viewport.style.scrollBehavior;
  viewport.style.scrollBehavior = "auto";
  update();
  viewport.style.scrollBehavior = previousBehavior;
}

/** Position the native viewport at the tail without invoking virtualizer scroll APIs (avoids multi-frame reconcile). */
export function snapLogViewportToBottom(viewport: HTMLElement, itemCount: number): void {
  const maxScrollTop = Math.max(0, estimatedLogListHeight(itemCount) - viewport.clientHeight);
  withInstantScroll(viewport, () => {
    viewport.scrollTop = maxScrollTop;
  });
}
