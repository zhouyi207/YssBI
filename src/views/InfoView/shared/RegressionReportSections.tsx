import type { ReactNode } from 'react';
import {
  StatCard,
  Chi2TestCards,
  BP_VARIANTS,
  FTestCards,
  OV_VARIANTS,
  formatNum,
} from './RegressionShared';
import { ModelSummaryGrid } from './ModelSummaryGrid';
import { AnovaTable } from './AnovaTable';
import { CoefficientsBlock } from './CoefficientsBlock';
import { ACFPACFBlock } from './ACFPACFBlock';
import { SerialTestsBlock } from './SerialTestsBlock';
import { VifTable, meanFiniteVif } from './VifTable';
import { ReportLazyBoundary, ReportSection, ReportSubheading } from './ReportLayout';
import { LazyKDE, LazyResidualPlot, LazyScatter } from './reportLazyModules';
import { HypothesisTestBlock } from './HypothesisTestBlock';
import type { DiagnosticInfo, RegressionResultData } from '@/shared/types/report';

export function OmittedVariablesAlert({ diag }: { diag: DiagnosticInfo }) {
  if (!diag.omit_info?.omitted.length) return null;

  return (
    <div className="mb-6 rounded-lg border border-amber-500/30 bg-amber-500/5 p-4">
      <div className="flex items-start gap-2">
        <svg
          className="mt-0.5 h-5 w-5 shrink-0 text-amber-400"
          fill="none"
          stroke="currentColor"
          viewBox="0 0 24 24"
        >
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth={2}
            d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-2.5L13.732 4c-.77-.833-1.964-.833-2.732 0L4.082 16.5c-.77.833.192 2.5 1.732 2.5z"
          />
        </svg>
        <div>
          <div className="mb-1 font-medium text-amber-400">Omitted variables (collinearity)</div>
          <div className="text-sm text-foreground">
            The following variables were dropped due to strict multicollinearity (non-dummy variables removed first):
          </div>
          <ul className="mt-2 space-y-1 font-mono text-sm">
            {diag.omit_info.omitted.map((o, i) => (
              <li key={i} className="text-muted-foreground">
                {o.variable}
                {o.category != null ? (
                  <span className="ml-1 rounded border border-indigo-500/25 px-1.5 py-0.5 text-indigo-300">
                    {o.category}
                  </span>
                ) : null}
                <span className="ml-1 text-xs text-muted-foreground">({o.reason})</span>
              </li>
            ))}
          </ul>
        </div>
      </div>
    </div>
  );
}

export function RegressionModelCoreSections({
  data,
  hasCategorical,
  coefficientsProps,
  showOmittedVariables = false,
}: {
  data: RegressionResultData;
  hasCategorical: boolean;
  coefficientsProps?: {
    useZStat?: boolean;
    ar1Rho?: number;
  };
  showOmittedVariables?: boolean;
}) {
  const { model_basic_info: info, coefficients, diagnostic_info: diag } = data;

  return (
    <>
      <ReportSection title="Model Summary" icon="modelSummary">
        <ModelSummaryGrid info={info} executionTimeMs={data.executionTimeMs} />
      </ReportSection>

      <ReportSection title="ANOVA" icon="anova">
        <AnovaTable info={info} />
      </ReportSection>

      <CoefficientsBlock
        coefficients={coefficients}
        hasCategorical={hasCategorical}
        useZStat={coefficientsProps?.useZStat}
        ar1Rho={coefficientsProps?.ar1Rho}
      />

      {showOmittedVariables ? <OmittedVariablesAlert diag={diag} /> : null}

      <HypothesisTestBlock data={data} />
    </>
  );
}

export function MulticollinearityBlock({ diag }: { diag: DiagnosticInfo }) {
  return (
    <div className="mb-4">
      <ReportSubheading title="Multicollinearity — Condition Number & VIF (Stata estat vif)" />
      <div className="mb-3 grid grid-cols-1 gap-3 sm:grid-cols-2">
        <StatCard
          label="Condition Number"
          value={formatNum(diag.cond_no)}
          sub={diag.cond_no > 1000 ? 'Possible multicollinearity' : 'Acceptable'}
        />
        {diag.vif && diag.vif.length > 0
          ? (() => {
              const meanVif = meanFiniteVif(diag.vif);
              return meanVif != null ? (
                <StatCard
                  label="Mean VIF"
                  value={formatNum(meanVif)}
                  sub={meanVif > 10 ? 'High multicollinearity' : meanVif > 5 ? 'Moderate' : 'Low'}
                />
              ) : null;
            })()
          : null}
      </div>
      {diag.vif && diag.vif.length > 0 ? <VifTable rows={diag.vif} /> : null}
    </div>
  );
}

export function OlsStyleDiagnosticsBlock({ diag }: { diag: DiagnosticInfo }) {
  return (
    <>
      <MulticollinearityBlock diag={diag} />

      {diag.bp_tests ? (
        <div className="mb-4">
          <ReportSubheading
            title="Breusch-Pagan (Heteroscedasticity) — Stata estat hettest 四种变体"
            timingMs={diag.timing?.bp_tests_ms}
          />
          <Chi2TestCards
            cards={BP_VARIANTS.filter(({ key }) => diag.bp_tests![key]).map(({ key, label }) => ({
              label,
              chi2: diag.bp_tests![key]!.lm_stat,
              df: diag.bp_tests![key]!.df,
              p_value: diag.bp_tests![key]!.p_value,
            }))}
          />
        </div>
      ) : null}

      {diag.ov_tests ? (
        <div className="mb-4">
          <ReportSubheading
            title="Ramsey RESET (Omitted Variables) — Stata estat ovtest 两种变体"
            timingMs={diag.timing?.ov_tests_ms}
          />
          <FTestCards
            cards={OV_VARIANTS.filter(({ key }) => diag.ov_tests![key]).map(({ key, label }) => ({
              label,
              f_stat: diag.ov_tests![key]!.f_stat,
              df1: diag.ov_tests![key]!.df1,
              df2: diag.ov_tests![key]!.df2,
              p_value: diag.ov_tests![key]!.p_value,
            }))}
          />
        </div>
      ) : null}

      {diag.im_test ? (
        <div className="mb-4">
          <ReportSubheading
            title="Cameron & Trivedi's decomposition of IM-test — Stata estat imtest"
            timingMs={diag.timing?.im_test_ms}
          />
          <Chi2TestCards
            cards={[
              { label: 'Heteroskedasticity', ...diag.im_test.heteroskedasticity },
              { label: 'Skewness', ...diag.im_test.skewness },
              { label: 'Kurtosis', ...diag.im_test.kurtosis },
              { label: 'Total', ...diag.im_test.total },
            ]}
          />
        </div>
      ) : null}

      {diag.normality_tests ? (
        <div className="mb-4">
          <ReportSubheading title="Residual Normality (Omnibus / Jarque-Bera)" />
          <Chi2TestCards
            cards={[
              {
                label: 'Omnibus',
                chi2: diag.normality_tests.omnibus_stat,
                df: 2,
                p_value: diag.normality_tests.omnibus_p_value,
              },
              {
                label: 'Jarque-Bera',
                chi2: diag.normality_tests.jarque_bera_stat,
                df: 2,
                p_value: diag.normality_tests.jarque_bera_p_value,
              },
            ]}
          />
        </div>
      ) : null}
    </>
  );
}

export interface ResidualDiagnosticsLabels {
  leverageTitle?: string;
  fittedTitle?: string;
  fittedTrailing?: ReactNode;
  scatterTitle?: string;
  scatterXLabel?: string;
  scatterYLabel?: string;
  acfResidualLabel?: string;
  serialTestsResidualLabel?: string;
  showNormalitySkewKurtosis?: boolean;
}

export function ResidualDiagnosticsSection({
  diag,
  leverageKdeData,
  labels = {},
}: {
  diag: DiagnosticInfo;
  leverageKdeData: ReturnType<typeof import('./utils').computeKDE>;
  labels?: ResidualDiagnosticsLabels;
}) {
  const {
    leverageTitle = 'Leverage KDE (Stata predict lev, leverage)',
    fittedTitle = 'Residuals vs Fitted',
    fittedTrailing,
    scatterTitle = 'Residuals: e vs e_lag1',
    scatterXLabel = 'e_{t-1}',
    scatterYLabel = 'e_t',
    acfResidualLabel,
    serialTestsResidualLabel,
    showNormalitySkewKurtosis = true,
  } = labels;

  if (!diag.fitted_values || !diag.residuals || diag.fitted_values.length === 0) {
    return null;
  }

  const scatterPoints =
    diag.residual_scatter?.e.length && diag.residual_scatter.e_lag1.length
      ? diag.residual_scatter.e_lag1.map((x, i) => ({ x, y: diag.residual_scatter!.e[i] }))
      : null;

  return (
    <>
      {diag.leverage && diag.leverage.length > 0 ? (
        <div className="mb-4">
          <ReportSubheading title={leverageTitle} />
          <ReportLazyBoundary variant="chart">
            <LazyKDE data={leverageKdeData} xLabel="Leverage" yLabel="Density" height={280} xMin={0} />
          </ReportLazyBoundary>
        </div>
      ) : null}

      <ReportSubheading
        title={fittedTitle}
        trailing={fittedTrailing}
        timingMs={fittedTrailing ? undefined : diag.timing?.fitted_residuals_ms}
      />
      <ReportLazyBoundary variant="chart">
        <LazyResidualPlot
          fitted={diag.fitted_values}
          residuals={diag.residuals}
          leverage={diag.leverage}
        />
      </ReportLazyBoundary>

      {scatterPoints ? (
        <div className="mt-4">
          <ReportSubheading title={scatterTitle} />
          <ReportLazyBoundary variant="chart">
            <LazyScatter
              data={scatterPoints}
              xLabel={scatterXLabel}
              yLabel={scatterYLabel}
              height={280}
              symmetricY
              zeroLine
            />
          </ReportLazyBoundary>
        </div>
      ) : null}

      <ACFPACFBlock residuals={diag.residuals} residualLabel={acfResidualLabel} />
      <SerialTestsBlock residuals={diag.residuals} exog={diag.exog} residualLabel={serialTestsResidualLabel} />

      {showNormalitySkewKurtosis && diag.normality_tests ? (
        <div className="mt-4">
          <ReportSubheading title="Skew & Kurtosis" />
          <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
            <div className="rounded-lg border border-border bg-muted px-4 py-3 transition-colors hover:border-border">
              <div className="mb-2 font-mono text-[11px] text-muted-foreground">Skew</div>
              <div className="font-mono text-sm font-medium text-foreground">
                {formatNum(diag.normality_tests.skewness)}
              </div>
            </div>
            <div className="rounded-lg border border-border bg-muted px-4 py-3 transition-colors hover:border-border">
              <div className="mb-2 font-mono text-[11px] text-muted-foreground">Kurtosis</div>
              <div className="font-mono text-sm font-medium text-foreground">
                {formatNum(diag.normality_tests.kurtosis)}
              </div>
            </div>
          </div>
        </div>
      ) : null}
    </>
  );
}

export function OlsStyleDiagnosticsSection({
  diag,
  leverageKdeData,
  residualLabels,
}: {
  diag: DiagnosticInfo;
  leverageKdeData: ReturnType<typeof import('./utils').computeKDE>;
  residualLabels?: ResidualDiagnosticsLabels;
}) {
  return (
    <ReportSection title="Diagnostics" icon="diagnostics">
      <OlsStyleDiagnosticsBlock diag={diag} />
      <ResidualDiagnosticsSection diag={diag} leverageKdeData={leverageKdeData} labels={residualLabels} />
    </ReportSection>
  );
}
