import { useMemo } from "react";
import type { RegressionResultData } from "@/shared/types/report";

export function useRegressionReport(data: RegressionResultData) {
  const { model_basic_info: info, coefficients, diagnostic_info: diag } = data;

  const hasCategorical = useMemo(
    () => coefficients.some((c) => c.category != null),
    [coefficients],
  );

  const leverageKdeData = diag.leverage_kde ?? [];

  const hasResidualSeries = Boolean(
    diag.fitted_values && diag.residuals && diag.fitted_values.length > 0,
  );

  const residualScatterPoints = useMemo(() => {
    if (!diag.residual_scatter?.e.length || !diag.residual_scatter.e_lag1.length) {
      return null;
    }
    return diag.residual_scatter.e_lag1.map((x, i) => ({
      x,
      y: diag.residual_scatter!.e[i],
    }));
  }, [diag.residual_scatter]);

  return {
    info,
    coefficients,
    diag,
    hasCategorical,
    leverageKdeData,
    hasResidualSeries,
    residualScatterPoints,
  };
}
