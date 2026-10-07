/** Distance from viewport bottom treated as following the live diagnostic tail. */
export const LOG_SCROLL_BOTTOM_THRESHOLD = 80;

export function isLogViewportPinnedToBottom(
  scrollTop: number,
  scrollHeight: number,
  clientHeight: number,
  threshold = LOG_SCROLL_BOTTOM_THRESHOLD,
): boolean {
  return scrollHeight - scrollTop - clientHeight < threshold;
}
