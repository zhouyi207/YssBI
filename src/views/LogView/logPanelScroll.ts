/** Distance from viewport bottom treated as "following the tail". */
export const LOG_SCROLL_BOTTOM_THRESHOLD = 80;

/** Scroll within this distance from top triggers loading older history. */
export const LOG_LOAD_OLDER_TOP_THRESHOLD = 150;

export function isLogViewportPinnedToBottom(
  scrollTop: number,
  scrollHeight: number,
  clientHeight: number,
  threshold = LOG_SCROLL_BOTTOM_THRESHOLD,
): boolean {
  return scrollHeight - scrollTop - clientHeight < threshold;
}

export function shouldLoadOlderLogs(
  scrollTop: number,
  threshold = LOG_LOAD_OLDER_TOP_THRESHOLD,
): boolean {
  return scrollTop < threshold;
}
