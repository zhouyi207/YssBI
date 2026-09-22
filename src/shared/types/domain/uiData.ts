export type UiValue = string | number;
export type UiValueFormat = "text" | "number" | "integer" | "pValue";

export interface UiMetric {
  readonly id: string;
  readonly label: string;
  readonly value: UiValue;
  readonly format: UiValueFormat;
}

export interface UiKeyValueData {
  readonly kind: "keyValue";
  readonly items: readonly UiMetric[];
}

export interface UiTableColumn {
  readonly id: string;
  readonly label: string;
  readonly format: UiValueFormat;
}

export interface UiTableData {
  readonly kind: "table";
  readonly columns: readonly UiTableColumn[];
  readonly rows: readonly (readonly UiValue[])[];
}

export interface UiStatCardData {
  readonly kind: "statCard";
  readonly stat: UiMetric;
}

export type UiDisplayData = UiKeyValueData | UiTableData | UiStatCardData;
