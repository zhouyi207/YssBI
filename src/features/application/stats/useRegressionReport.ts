import { useMemo } from "react";
import type { RegressionResultData } from "@/shared/types/report";

export function useRegressionReport<ModelInfo>(data: RegressionResultData<ModelInfo>) {
  const { model_basic_info: info, coefficients, diagnostic_info: diag } = data;

  const hasCategorical = useMemo(
    () => coefficients.some((c) => c.category != null),
    [coefficients],
  );

  const leverageKdeData = diag.leverage_kde ?? [];

  return {
    info,
    coefficients,
    diag,
    hasCategorical,
    leverageKdeData,
  };
}
