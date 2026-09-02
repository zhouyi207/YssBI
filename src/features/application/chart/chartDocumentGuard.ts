import type { ChartDocument } from "@/shared/types/domain/chart";

export function isChartDocument(value: unknown): value is ChartDocument {
  if (!value || typeof value !== "object") return false;
  const candidate = value as Partial<ChartDocument>;
  if (!Number.isSafeInteger(candidate.schemaVersion) || (candidate.schemaVersion ?? -1) < 0) {
    return false;
  }
  if (!Number.isSafeInteger(candidate.revision) || (candidate.revision ?? -1) < 0) return false;
  if (typeof candidate.databaseId !== "string") return false;
  if (
    candidate.chartType !== "histogram" &&
    candidate.chartType !== "scatter" &&
    candidate.chartType !== "line"
  ) {
    return false;
  }
  if (!candidate.encodings || typeof candidate.encodings !== "object") return false;
  return (
    (candidate.encodings.x === undefined || typeof candidate.encodings.x === "string") &&
    (candidate.encodings.y === undefined || typeof candidate.encodings.y === "string")
  );
}
