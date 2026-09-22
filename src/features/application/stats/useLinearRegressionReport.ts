import { useCallback, useMemo } from "react";
import type { LinearRegressionReportData } from "@/shared/types/domain/resultReport";
import type { Coefficient } from "@/shared/types/report/regression";
import { linearCoefficientField } from "@/shared/types/report/parseLinearRegression";
import { usePagedResultRows } from "@/features/application/results/usePagedResultRows";
import { useResultAnalysis } from "@/features/application/results/useResultAnalysis";
import type { HypothesisTestSource } from "./useHypothesisTestBlock";

export const LINEAR_COEFFICIENT_PAGE_SIZE = 200;

export function useLinearRegressionCoefficients(data: LinearRegressionReportData) {
  const page = usePagedResultRows(
    data.resultRef,
    data.coefficients.rowCount,
    LINEAR_COEFFICIENT_PAGE_SIZE,
    undefined,
    data.coefficients.part,
  );
  const parsed = useMemo(() => {
    const coefficients: Coefficient[] = [];
    for (const row of page.rows) {
      const value = Object.fromEntries(
        page.columns.map((column, index) => [column.name, row[index]]),
      );
      const parsed = linearCoefficientField.read(value, "coefficients");
      if (!parsed.ok)
        return {
          coefficients: [],
          error: { code: "invalid_result_coefficients", incidentId: null },
        };
      coefficients.push(parsed.value);
    }
    return { coefficients, error: null };
  }, [page.rows, page.columns]);

  return useMemo(
    () => ({ page, coefficients: parsed.coefficients, coefficientError: parsed.error }),
    [page, parsed],
  );
}

export function useLinearRegressionAnalysis(data: LinearRegressionReportData) {
  const analyze = useResultAnalysis(data.resultRef);
  const hypothesisSource = useMemo<HypothesisTestSource>(
    () => ({
      paramNames: data.paramNames,
      available: data.coefficients.rowCount > 0,
      test: async (hypothesis) => {
        const response = await analyze({ kind: "hypothesis", hypothesis });
        return response?.kind === "hypothesis" ? response.value : null;
      },
    }),
    [analyze, data.paramNames, data.coefficients.rowCount],
  );
  const computeAcf = useCallback(
    async (maxLag: number) => {
      const response = await analyze({ kind: "acfPacf", maxLag });
      return response?.kind === "acfPacf"
        ? { acf: [...response.value.acf], pacf: [...response.value.pacf], n: response.value.n }
        : null;
    },
    [analyze],
  );
  const computeSerialTests = useCallback(
    async (lags: number, bgNomiss0: boolean) => {
      const response = await analyze({ kind: "serialTests", lags, bgNomiss0 });
      return response?.kind === "serialTests" ? response.value : null;
    },
    [analyze],
  );
  return { hypothesisSource, computeAcf, computeSerialTests };
}
