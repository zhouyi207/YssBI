import { isResultReference, type ResultReference } from "@/shared/types/domain/result";

export interface PresentationWindowQuery {
  reference: ResultReference | null;
  leaseId: string | null;
  plotType: string | null;
}

function readLocationQueryString(hash: string, search: string): string {
  const queryStart = hash.indexOf("?");
  if (queryStart >= 0) return hash.slice(queryStart + 1);
  return search.startsWith("?") ? search.slice(1) : search;
}

export function parsePresentationWindowQueryFromParts(
  hash: string,
  search = "",
): PresentationWindowQuery {
  const params = new URLSearchParams(readLocationQueryString(hash, search));
  const reference = {
    executionSessionId: params.get("executionSessionId"),
    resultId: params.get("resultId"),
  };
  return {
    reference: isResultReference(reference) ? reference : null,
    leaseId: params.get("leaseId"),
    plotType: params.get("plotType"),
  };
}

export function parsePresentationWindowQuery(): PresentationWindowQuery {
  return parsePresentationWindowQueryFromParts(window.location.hash, window.location.search);
}

export function parsePlotChartFromLocation(): string | null {
  return parsePresentationWindowQuery().plotType;
}
