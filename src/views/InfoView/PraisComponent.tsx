import type { FC } from "react";
import { ScrollArea } from "@/components/ui/scroll-area";
import { useRegressionReport } from "@/features/application/stats/useRegressionReport";
import {
  ReportLayout,
  ReportLazyBoundary,
  ReportSection,
  ReportSubheading,
  RSquaredBadge,
  StatCard,
  formatNum,
  LazyFormulaBlock,
  RegressionModelCoreSections,
  MulticollinearityBlock,
  ResidualDiagnosticsSection,
} from "./shared";
import type { RegressionResultData } from "@/shared/types/report";

export interface PraisResultData extends RegressionResultData {
  diagnostic_info: RegressionResultData["diagnostic_info"] & {
    prais_info: {
      rho: number;
      dw_original: number;
      dw_transformed: number;
      iterations: number;
      iteration_log?: string[];
    };
  };
}

export const PraisComponent: FC<{ data: PraisResultData }> = ({ data }) => {
  const { info, coefficients, diag, hasCategorical, leverageKdeData } = useRegressionReport(data);
  const praisInfo = diag.prais_info!;

  return (
    <ReportLayout
      title={data.title}
      badges={
        <>
          <RSquaredBadge value={info.r_squared} />
          <span className="text-xs text-muted-foreground">
            {info.method} &middot; n={info.num_observation} &middot; ρ={formatNum(praisInfo.rho)}
          </span>
        </>
      }
    >
      <ReportSection title="Equation" icon="equation">
        <ReportLazyBoundary variant="formula">
          <LazyFormulaBlock
            endogName={data.endog_name || "y"}
            coefficients={coefficients}
            ar1Rho={praisInfo.rho}
          />
        </ReportLazyBoundary>
      </ReportSection>

      <RegressionModelCoreSections
        data={data}
        hasCategorical={hasCategorical}
        coefficientsProps={{ ar1Rho: praisInfo.rho }}
        showOmittedVariables
      />

      <ReportSection title="AR(1) Diagnostics" icon="diagnostics">
        <div className="mb-4 space-y-3">
          <div className="grid grid-cols-2 gap-3">
            <StatCard
              label="DW (original)"
              value={formatNum(praisInfo.dw_original)}
              sub="检验对象: 初始 OLS 残差 u_t"
            />
            <StatCard
              label="DW (transformed)"
              value={formatNum(praisInfo.dw_transformed)}
              sub="检验对象: 变换后残差 e_t"
            />
          </div>
          <StatCard label="Iterations" value={praisInfo.iterations} sub="Convergence" />
          <MulticollinearityBlock diag={diag} />
        </div>

        {praisInfo.iteration_log && praisInfo.iteration_log.length > 0 ? (
          <div className="mb-4 overflow-hidden rounded-lg border border-border bg-card">
            <ReportSubheading title="Iteration Log" />
            <ScrollArea orientation="both" className="max-h-40">
              <pre className="min-w-max whitespace-pre px-4 py-3 font-mono text-xs text-foreground">
                {praisInfo.iteration_log.join("\n")}
              </pre>
            </ScrollArea>
          </div>
        ) : null}

        <ResidualDiagnosticsSection
          diag={diag}
          leverageKdeData={leverageKdeData}
          labels={{
            fittedTitle: "Residuals vs Fitted",
            fittedTrailing: (
              <span className="text-[10px] text-muted-foreground">
                检验对象: u_t (Prais 收敛后)
              </span>
            ),
            scatterTitle: "Residuals: u_t vs u_{t-1}",
            scatterXLabel: "u_{t-1}",
            scatterYLabel: "u_t",
            acfResidualLabel: "u_t (Prais 收敛后)",
            serialTestsResidualLabel: "u_t (Prais 收敛后)",
            showNormalitySkewKurtosis: false,
          }}
        />
      </ReportSection>
    </ReportLayout>
  );
};
