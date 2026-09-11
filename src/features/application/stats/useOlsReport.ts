import { useMemo } from "react";
import type { OlsReportData } from "@/shared/types/domain/resultReport";
import type { Coefficient } from "@/shared/types/report/regression";
import { olsCoefficientField } from "@/shared/types/report/parseOls";
import { usePagedResultRows } from "@/features/application/results/usePagedResultRows";
import { useResultAnalysis } from "@/features/application/results/useResultAnalysis";
import type { HypothesisTestSource } from "./useHypothesisTestBlock";

export function useOlsReport(data: OlsReportData) {
  const page = usePagedResultRows(
    data.resultRef,
    data.coefficients.rowCount,
    200,
    undefined,
    data.coefficients.part,
  );
  const analyze = useResultAnalysis(data.resultRef);
  const parsed = useMemo(() => {
    const coefficients: Coefficient[] = [];
    for (const row of page.rows) {
      const value = Object.fromEntries(
        page.columns.map((column, index) => [column.name, row[index]]),
      );
      const parsed = olsCoefficientField.read(value, "coefficients");
      if (!parsed.ok)
        return {
          coefficients: [],
          error: { code: "invalid_result_coefficients", incidentId: null },
        };
      coefficients.push(parsed.value);
    }
    return { coefficients, error: null };
  }, [page.rows, page.columns]);

  const hypothesisSource: HypothesisTestSource = {
    paramNames: parsed.coefficients.map((coefficient) => coefficient.variable),
    available: data.coefficients.rowCount > 0,
    test: async (hypothesis) => {
      const response = await analyze({ kind: "hypothesis", hypothesis });
      return response?.kind === "hypothesis" ? response.value : null;
    },
  };

  return {
    page,
    coefficients: parsed.coefficients,
    coefficientError: parsed.error,
    hypothesisSource,
    computeAcf: async (maxLag: number) => {
      const response = await analyze({ kind: "acfPacf", maxLag });
      return response?.kind === "acfPacf"
        ? { acf: [...response.value.acf], pacf: [...response.value.pacf], n: response.value.n }
        : null;
    },
    computeSerialTests: async (lags: number, bgNomiss0: boolean) => {
      const response = await analyze({ kind: "serialTests", lags, bgNomiss0 });
      return response?.kind === "serialTests" ? response.value : null;
    },
  };
}
