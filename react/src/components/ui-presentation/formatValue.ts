import type { UiValue, UiValueFormat } from "@/shared/types/domain/uiData";
import { formatNum } from "@/shared/stats/formatStat";

export function formatValue(value: UiValue, format: UiValueFormat): string {
  return format === "number" || format === "pValue" ? formatNum(value) : String(value);
}

export function valueClass(value: UiValue, format: UiValueFormat): string {
  return format === "pValue" && typeof value === "number" && value < 0.05
    ? "text-emerald-400"
    : "text-foreground";
}
