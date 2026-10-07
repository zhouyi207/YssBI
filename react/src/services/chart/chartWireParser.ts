import { z } from "zod";
import { isChartDocumentState, type ChartDocument } from "@/shared/types/domain/chart";

const chartDocumentEnvelope = z.strictObject({
  schemaVersion: z.literal(4),
  databaseId: z.unknown(),
  chartType: z.unknown(),
  encodings: z.unknown(),
});

export function parseChartDocument(value: unknown): ChartDocument {
  const { schemaVersion, ...state } = chartDocumentEnvelope.parse(value);
  if (!isChartDocumentState(state)) throw new TypeError("Invalid chart document state");
  return { schemaVersion, ...state, encodings: { ...state.encodings } };
}

const axisFormat = z.enum(["date", "datetime", "number"]);
export const plotColumnPairSchema = z.strictObject({
  data: z.array(z.strictObject({ x: z.number(), y: z.number() })),
  xLabel: z.string().nullable(),
  yLabel: z.string().nullable(),
  xFormat: axisFormat,
  yFormat: axisFormat,
});
