import type { FC } from "react";
import { useRegressionReport } from "@/features/application/stats/useRegressionReport";
import {
  ReportLayout,
  ReportLazyBoundary,
  ReportSection,
  LazyBinaryFormulaBlock,
  LazyScatter,
  BinaryModelSummaryGrid,
  ClassificationTableBlock,
  CoefficientsBlock,
  HypothesisTestBlock,
  MarginsBlock,
  formatNum,
} from "./shared";
import type { OLSResultData } from "@/shared/types/report";

export type { OLSResultData };

/** Binary choice model component (Logit, Probit) */
export const BinaryComponent: FC<{ data: OLSResultData }> = ({ data }) => {
  const { info, coefficients, diag, hasCategorical } = useRegressionReport(data);

  return (
    <ReportLayout
      title={data.title}
      badges={
        <>
          <span className="inline-flex items-center rounded-full border border-emerald-500/30 bg-emerald-500/20 px-2.5 py-0.5 text-xs font-semibold text-emerald-400">
            Pseudo R² = {formatNum(info.r_squared, 3)}
          </span>
          <span className="text-xs text-muted-foreground">
            {info.method} &middot; n={info.num_observation}
          </span>
        </>
      }
    >
      <ReportSection title="Equation" icon="equation">
        <ReportLazyBoundary variant="formula">
          <LazyBinaryFormulaBlock
            modelType={info.model_type === "Probit" ? "Probit" : "Logit"}
            endogName={data.endog_name || "y"}
            coefficients={coefficients}
          />
        </ReportLazyBoundary>
      </ReportSection>

      <ReportSection title="Model Summary" icon="modelSummary">
        <BinaryModelSummaryGrid info={info} executionTimeMs={data.executionTimeMs} />
      </ReportSection>

      {diag.classification_table ? (
        <ReportSection title="Classification Table" icon="classification">
          <ClassificationTableBlock data={diag.classification_table} />
        </ReportSection>
      ) : null}

      <CoefficientsBlock
        coefficients={coefficients}
        hasCategorical={hasCategorical}
        useZStat
        showOddsRatio={info.model_type === "Logit"}
      />

      <MarginsBlock data={data} />
      <HypothesisTestBlock data={data} />

      {diag.fitted_values && diag.residuals && diag.fitted_values.length > 0 ? (
        <ReportSection title="Residuals vs Fitted (Probabilities)" icon="anova">
          <ReportLazyBoundary variant="chart">
            <LazyScatter
              data={diag.fitted_values.map((x, i) => ({ x, y: (diag.residuals ?? [])[i] ?? 0 }))}
              xAxis={{ label: "Fitted (P)", valueType: "number" }}
              yAxis={{ label: "Residual (y - P)", valueType: "number" }}
              height={280}
              symmetricY
              zeroLine
            />
          </ReportLazyBoundary>
        </ReportSection>
      ) : null}
    </ReportLayout>
  );
};
