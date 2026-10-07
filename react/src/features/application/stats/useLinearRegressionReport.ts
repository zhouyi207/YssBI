import { useMemo } from "react";
import type { LinearRegressionReportData } from "@/shared/types/domain/resultReport";
import type { Coefficient } from "@/shared/types/report/regression";
import { linearCoefficientField } from "@/shared/types/report/parseLinearRegression";
import { usePagedResultRows } from "@/features/application/results/usePagedResultRows";

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
