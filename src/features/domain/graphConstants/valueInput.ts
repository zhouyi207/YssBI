import { z } from "zod";
import { dataValueFromRaw, type DataValue } from "@/shared/types/domain/dataValue";
import type { ValueType } from "@/shared/types/domain/valueType";

export type ConstantValueInputError =
  | "invalidJson"
  | "invalidValue"
  | "notArray"
  | "notObject"
  | "notDataFrameContent"
  | "notDataSeriesContent";

export type ConstantValueInputResult =
  | { ok: true; value: DataValue }
  | { ok: false; error: ConstantValueInputError };

export function parseConstantValueInput(
  raw: unknown,
  dataType: ValueType,
): ConstantValueInputResult {
  try {
    return { ok: true, value: dataValueFromRaw(raw, dataType) };
  } catch {
    return { ok: false, error: "invalidValue" };
  }
}

const tableColumnsSchema = z.array(
  z.array(z.union([z.null(), z.boolean(), z.number().finite(), z.string()])),
);

function isColumnMap(value: unknown): value is Record<string, unknown[]> {
  return (
    value !== null &&
    typeof value === "object" &&
    !Array.isArray(value) &&
    Object.entries(value).every(([key, cells]) => key.trim().length > 0 && Array.isArray(cells))
  );
}

export function parseConstantJsonInput(
  json: string,
  dataType: ValueType,
): ConstantValueInputResult {
  let parsed: unknown;
  try {
    parsed = JSON.parse(json);
  } catch {
    return { ok: false, error: "invalidJson" };
  }

  switch (dataType.kind) {
    case "Array":
      return Array.isArray(parsed)
        ? parseConstantValueInput(parsed, dataType)
        : { ok: false, error: "notArray" };
    case "Object":
      return parsed !== null && typeof parsed === "object" && !Array.isArray(parsed)
        ? parseConstantValueInput(parsed, dataType)
        : { ok: false, error: "notObject" };
    case "DataFrame":
    case "DataSeries": {
      const error = dataType.kind === "DataFrame" ? "notDataFrameContent" : "notDataSeriesContent";
      if (!isColumnMap(parsed)) return { ok: false, error };
      const columns = Object.values(parsed);
      if (
        !tableColumnsSchema.safeParse(columns).success ||
        (dataType.kind === "DataSeries" && columns.length > 1) ||
        columns.some((column) => column.length !== columns[0].length)
      )
        return { ok: false, error };
      return {
        ok: true,
        value:
          columns.length === 0
            ? { kind: "Null" }
            : { kind: dataType.kind, value: JSON.stringify(parsed) },
      };
    }
    default:
      return { ok: false, error: "invalidValue" };
  }
}
