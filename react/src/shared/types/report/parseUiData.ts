import type { UiMetric, UiTableData, UiValue, UiValueFormat } from "../domain/uiData";
import {
  arrayField,
  literalField,
  objectField,
  parsedField,
  refineField,
  stringField,
} from "./fields";

const valueField = parsedField<UiValue>(
  (value) =>
    typeof value === "string" || (typeof value === "number" && Number.isFinite(value))
      ? value
      : null,
  "string or finite number",
);
const formatField = literalField("text", "number", "integer", "pValue");

function matchesFormat(value: UiValue, format: UiValueFormat) {
  if (format === "text") return typeof value === "string";
  if (typeof value !== "number") return false;
  if (format === "integer") return Number.isSafeInteger(value);
  return format !== "pValue" || (value >= 0 && value <= 1);
}

const metricField = refineField(
  objectField<UiMetric>({
    id: stringField,
    label: stringField,
    value: valueField,
    format: formatField,
  }),
  (value) =>
    matchesFormat(value.value, value.format)
      ? null
      : { fieldPath: "value", reason: "value does not match its format" },
);

export const keyValueDataField = refineField(
  objectField({
    kind: literalField("keyValue"),
    items: arrayField(metricField),
  }),
  (value) =>
    new Set(value.items.map((item) => item.id)).size === value.items.length
      ? null
      : { fieldPath: "items", reason: "duplicate metric id" },
);

export const tableDataField = refineField(
  objectField<UiTableData>({
    kind: literalField("table"),
    columns: arrayField(objectField({ id: stringField, label: stringField, format: formatField })),
    rows: arrayField(arrayField(valueField)),
  }),
  (value) => {
    if (
      !value.columns.length ||
      new Set(value.columns.map((column) => column.id)).size !== value.columns.length
    )
      return { fieldPath: "columns", reason: "expected unique, non-empty columns" };
    for (let index = 0; index < value.rows.length; index++) {
      const row = value.rows[index];
      if (
        row.length !== value.columns.length ||
        !row.every((cell, column) => matchesFormat(cell, value.columns[column].format))
      )
        return { fieldPath: `rows[${index}]`, reason: "row does not match its columns" };
    }
    return null;
  },
);

export const statCardDataField = objectField({ kind: literalField("statCard"), stat: metricField });
