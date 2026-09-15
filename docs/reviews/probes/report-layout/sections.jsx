import { createContext, useContext } from "react";
import { TooltipProvider } from "@/components/ui/tooltip";
import { ModelSummaryGrid } from "@/modules/results/internal/ui/info/shared/ModelSummaryGrid";
import { CoefficientsBlock } from "@/modules/results/internal/ui/info/shared/CoefficientsBlock";
import { ReportSection } from "@/modules/results/internal/ui/info/shared/ReportLayout";
import { ScatterChart } from "@/shared/charts/cartesian/ScatterChart";
import { ChartThemeProvider } from "@/app/providers/ChartThemeProvider";
import { olsCoefficientField, olsReportField } from "@/shared/types/report/parseOls";

const Result = createContext(null);

export function ResultProvider({ sample, children }) {
  return (
    <Result.Provider value={sample}>
      <TooltipProvider>
        <ChartThemeProvider>{children}</ChartThemeProvider>
      </TooltipProvider>
    </Result.Provider>
  );
}

export function prepareSample(raw) {
  const report = olsReportField.read(raw.report, "$");
  if (!report.ok) throw new Error(JSON.stringify(report.issue));
  const coefficients = raw.coefficients.values.map((row) => {
    const value = Object.fromEntries(
      raw.coefficients.metadata.columns.map((column, index) => [column.name, row[index]]),
    );
    const parsed = olsCoefficientField.read(value, "coefficients");
    if (!parsed.ok) throw new Error(JSON.stringify(parsed.issue));
    return parsed.value;
  });
  return { report: report.value, coefficients, points: raw.residualPlot.value.points };
}

// Both candidates use exactly these production section components and the same Result DTOs.
export function Section({ kind }) {
  const sample = useContext(Result);
  switch (kind) {
    case "modelSummary":
      return (
        <ReportSection title="Model Summary" icon="modelSummary">
          <ModelSummaryGrid info={sample.report.model_basic_info} />
        </ReportSection>
      );
    case "coefficientTable":
      return <CoefficientsBlock coefficients={sample.coefficients} hasCategorical={false} />;
    case "residualPlot":
      return (
        <ReportSection title="Residuals vs Fitted" icon="test">
          <ScatterChart
            data={sample.points}
            xAxis={{ label: "Fitted Values", valueType: "number" }}
            yAxis={{ label: "Residuals", valueType: "number" }}
            height={280}
            symmetricY
            zeroLine
          />
        </ReportSection>
      );
    default:
      throw new Error(`Unsupported probe section: ${kind}`);
  }
}
