import type { Coefficient } from "@/shared/types/report";

/** 从系数表构建 param_names（与 exog 列序一致） */
export function buildParamNames(coefficients: Coefficient[]): string[] {
  return coefficients.map((c) => (c.category != null ? `${c.variable}_${c.category}` : c.variable));
}
