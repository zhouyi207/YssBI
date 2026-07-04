/** Read `sourceId` from hash-router query (`#/route?sourceId=...`) or search params. */
export function parseSourceIdFromLocation(): string | null {
  const searchValue = new URLSearchParams(window.location.search).get('sourceId');
  if (searchValue) return searchValue;

  const hash = window.location.hash;
  const match = hash.match(/[?&]sourceId=([^&]+)/);
  return match ? decodeURIComponent(match[1]) : null;
}

/** Fallback plot chart from URL when opening legacy windows. */
export function parsePlotChartFromLocation(): string | null {
  const hash = window.location.hash;
  const match = hash.match(/[?&]plotType=([^&]+)/);
  return match ? decodeURIComponent(match[1]) : null;
}
