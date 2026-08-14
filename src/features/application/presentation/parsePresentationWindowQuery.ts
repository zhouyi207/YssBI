export interface PresentationWindowQuery {
  resultId: string | null;
  plotType: string | null;
}

function readLocationQueryString(hash: string, search: string): string {
  const queryStart = hash.indexOf('?');
  if (queryStart >= 0) return hash.slice(queryStart + 1);
  return search.startsWith('?') ? search.slice(1) : search;
}

export function parsePresentationWindowQueryFromParts(
  hash: string,
  search = '',
): PresentationWindowQuery {
  const params = new URLSearchParams(readLocationQueryString(hash, search));
  return { resultId: params.get('resultId'), plotType: params.get('plotType') };
}

export function parsePresentationWindowQuery(): PresentationWindowQuery {
  return parsePresentationWindowQueryFromParts(window.location.hash, window.location.search);
}

export function parsePlotChartFromLocation(): string | null {
  return parsePresentationWindowQuery().plotType;
}
