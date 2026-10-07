import { utcFormat } from "d3";
import type { AxisValueType } from "@/shared/charts/ChartModel";

/** Date is an arithmetic carrier for naive day/microsecond values; never localize its fields. */
export function numToPlotDate(v: number, format: "date" | "datetime"): Date {
  if (format === "date") {
    return new Date(v * 86400000);
  }
  return new Date(v / 1000);
}

/** D3 轴 tick 格式化；`number` 时返回 undefined（使用默认格式）。 */
export function plotAxisTickFormatter(
  format: AxisValueType,
): ((value: { valueOf(): number }) => string) | undefined {
  if (format === "date") {
    return (d) => utcFormat("%Y-%m-%d")(numToPlotDate(Number(d.valueOf()), "date"));
  }
  if (format === "datetime") {
    return (d) => utcFormat("%Y-%m-%d %H:%M")(numToPlotDate(Number(d.valueOf()), "datetime"));
  }
  return undefined;
}
