import { Chi2TestCards, formatNum } from './RegressionShared';
import { CoefficientTable } from './CoefficientTable';
import { IvFirstStageSummaryTables } from './IvFirstStageSummaryTables';
import { ReportLazyBoundary, ReportSection } from './ReportLayout';
import { LazyFormulaBlock, LazyFormulaBlock2SLS } from './reportLazyModules';
import type { Coefficient, DiagnosticInfo } from './types';

type IvVariant = '2sls' | 'liml';

function IvEquationSection({
  endogName,
  coefficients,
  firstStage,
}: {
  endogName: string;
  coefficients: Coefficient[];
  firstStage: DiagnosticInfo['iv2sls_first_stage'];
}) {
  return (
    <ReportSection title="Equation" icon="equation">
      <ReportLazyBoundary variant="formula">
        {firstStage && firstStage.length > 0 ? (
          <LazyFormulaBlock2SLS
            endogName={endogName}
            coefficients={coefficients}
            firstStage={firstStage}
          />
        ) : (
          <LazyFormulaBlock endogName={endogName} coefficients={coefficients} />
        )}
      </ReportLazyBoundary>
    </ReportSection>
  );
}

function IvFirstStageResultsSection({
  firstStage,
}: {
  firstStage: NonNullable<DiagnosticInfo['iv2sls_first_stage']>;
}) {
  return (
    <ReportSection title="First Stage Regression Results" icon="firstStage">
      <div className="space-y-4">
        {firstStage.map((fs) => (
          <div key={fs.endog_name} className="overflow-hidden rounded-lg border border-border bg-muted">
            <div className="flex items-center justify-between border-b border-border px-4 py-2.5">
              <span className="text-sm font-medium text-foreground">{fs.endog_name} on exog + instruments</span>
              <span className="text-xs text-muted-foreground">
                R² = {fs.r_squared.toFixed(4)} · Adj R² = {fs.adj_r_squared.toFixed(4)}
              </span>
            </div>
            <CoefficientTable coefficients={fs.coefficients} hasCategorical={false} />
          </div>
        ))}
      </div>
    </ReportSection>
  );
}

function IvFirstStageSummarySection({
  variant,
  summary,
  firstStage,
}: {
  variant: IvVariant;
  summary: NonNullable<DiagnosticInfo['iv2sls_first_stage_summary']>;
  firstStage: DiagnosticInfo['iv2sls_first_stage'];
}) {
  const title =
    variant === 'liml'
      ? 'First Stage Summary (estat firststage) — LIML'
      : 'First Stage Summary (estat firststage)';

  return (
    <ReportSection title={title} icon="firstStage">
      <IvFirstStageSummaryTables summary={summary} firstStage={firstStage} variant={variant} />
    </ReportSection>
  );
}

function Iv2slsOveridSection({ diag }: { diag: DiagnosticInfo }) {
  if (!diag.iv2sls_overid_dims) return null;

  return (
    <ReportSection title="Overidentification Test (estat overid)" icon="test">
      <div className="mb-4">
        {diag.iv2sls_overid ? (
          <>
            <div className="mb-2 px-1">
              <span className="text-[11px] uppercase tracking-wider text-muted-foreground">
                Tests of overidentifying restrictions (df = {diag.iv2sls_overid.df})
                {diag.iv2sls_overid.test_type === 'wooldridge' && ' — Wooldridge score (robust)'}
              </span>
            </div>
            <Chi2TestCards
              cards={
                diag.iv2sls_overid.test_type === 'wooldridge'
                  ? [
                      {
                        label: 'Wooldridge score',
                        chi2: diag.iv2sls_overid.wooldridge_stat ?? 0,
                        df: diag.iv2sls_overid.df,
                        p_value: diag.iv2sls_overid.wooldridge_p_value ?? 0,
                      },
                    ]
                  : [
                      {
                        label: 'Sargan',
                        chi2: diag.iv2sls_overid.sargan_stat ?? 0,
                        df: diag.iv2sls_overid.df,
                        p_value: diag.iv2sls_overid.sargan_p_value ?? 0,
                      },
                      {
                        label: 'Basmann',
                        chi2: diag.iv2sls_overid.basmann_stat ?? 0,
                        df: diag.iv2sls_overid.df,
                        p_value: diag.iv2sls_overid.basmann_p_value ?? 0,
                      },
                    ]
              }
            />
            <p className="mt-2 px-1 text-xs text-muted-foreground">
              H0: overidentifying restrictions are valid. Significant p-value suggests instruments may not be valid.
              {diag.iv2sls_overid.test_type === 'wooldridge' &&
                ' Wooldridge (1995) score test is used with robust VCE (Sargan/Basmann assume homoskedasticity).'}
            </p>
          </>
        ) : (
          <div className="rounded-lg border border-amber-500/30 bg-amber-500/10 px-4 py-3">
            <p className="text-sm text-amber-200">
              Model is exactly identified (k_iv = {diag.iv2sls_overid_dims.k_iv}, k_endog ={' '}
              {diag.iv2sls_overid_dims.k_endog}).
            </p>
            <p className="mt-1 text-xs text-muted-foreground">
              The overidentification test requires k_iv &gt; k_endog (excluded instruments &gt; endogenous variables).
              Exogenous variables are not counted as instruments.
            </p>
          </div>
        )}
      </div>
    </ReportSection>
  );
}

function IvLimlOveridSection({ diag }: { diag: DiagnosticInfo }) {
  if (!diag.iv2sls_overid_dims) return null;

  return (
    <ReportSection title="Overidentification Test (estat overid)" icon="test">
      <div className="mb-4">
        {diag.ivliml_overid ? (
          <>
            <div className="mb-2 px-1">
              <span className="text-[11px] uppercase tracking-wider text-muted-foreground">
                Tests of overidentifying restrictions (df = {diag.ivliml_overid.df})
              </span>
            </div>
            <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
              <div className="rounded-lg border border-border bg-muted px-4 py-3 transition-colors hover:border-border">
                <div className="mb-2 font-mono text-[11px] text-muted-foreground">
                  Anderson-Rubin chi2({diag.ivliml_overid.df})
                </div>
                <div className="flex flex-wrap items-baseline gap-x-4 gap-y-1 text-xs">
                  <span className="text-muted-foreground">
                    chi2 ={' '}
                    <span className="font-mono text-foreground">
                      {formatNum(diag.ivliml_overid.anderson_rubin_stat)}
                    </span>
                  </span>
                  <span className="text-muted-foreground">
                    df = <span className="font-mono text-foreground">{diag.ivliml_overid.df}</span>
                  </span>
                  <span className="text-muted-foreground">
                    p ={' '}
                    <span
                      className={`font-mono ${diag.ivliml_overid.anderson_rubin_p_value < 0.05 ? 'text-emerald-400' : 'text-muted-foreground'}`}
                    >
                      {formatNum(diag.ivliml_overid.anderson_rubin_p_value)}
                    </span>
                  </span>
                </div>
                <div className="mt-1.5 text-[10px]">
                  {diag.ivliml_overid.anderson_rubin_p_value < 0.05 ? (
                    <span className="text-amber-400">Reject H0 — instruments may not be valid</span>
                  ) : (
                    <span className="text-muted-foreground">
                      Do not reject H0 — overidentifying restrictions appear valid
                    </span>
                  )}
                </div>
              </div>
              <div className="rounded-lg border border-border bg-muted px-4 py-3 transition-colors hover:border-border">
                <div className="mb-2 font-mono text-[11px] text-muted-foreground">
                  Basmann F({diag.ivliml_overid.df},{diag.ivliml_overid.df_denom})
                </div>
                <div className="flex flex-wrap items-baseline gap-x-4 gap-y-1 text-xs">
                  <span className="text-muted-foreground">
                    F = <span className="font-mono text-foreground">{formatNum(diag.ivliml_overid.basmann_stat)}</span>
                  </span>
                  <span className="text-muted-foreground">
                    p ={' '}
                    <span
                      className={`font-mono ${diag.ivliml_overid.basmann_p_value < 0.05 ? 'text-emerald-400' : 'text-muted-foreground'}`}
                    >
                      {formatNum(diag.ivliml_overid.basmann_p_value)}
                    </span>
                  </span>
                </div>
                <div className="mt-1.5 text-[10px]">
                  {diag.ivliml_overid.basmann_p_value < 0.05 ? (
                    <span className="text-amber-400">Reject H0 — instruments may not be valid</span>
                  ) : (
                    <span className="text-muted-foreground">
                      Do not reject H0 — overidentifying restrictions appear valid
                    </span>
                  )}
                </div>
              </div>
            </div>
            <p className="mt-2 px-1 text-xs text-muted-foreground">
              H0: overidentifying restrictions are valid. Significant p-value suggests instruments may not be valid.
            </p>
          </>
        ) : (
          <div className="rounded-lg border border-amber-500/30 bg-amber-500/10 px-4 py-3">
            <p className="text-sm text-amber-200">
              Model is{' '}
              {diag.iv2sls_overid_dims.k_iv > diag.iv2sls_overid_dims.k_endog ? 'overidentified' : 'exactly identified'}{' '}
              (k_iv = {diag.iv2sls_overid_dims.k_iv}, k_endog = {diag.iv2sls_overid_dims.k_endog}).
            </p>
            <p className="mt-1 text-xs text-muted-foreground">
              {diag.iv2sls_overid_dims.k_iv <= diag.iv2sls_overid_dims.k_endog
                ? 'Overidentification test requires k_iv > k_endog.'
                : 'Overidentification test requires nonrobust VCE (homoskedastic errors).'}
            </p>
          </div>
        )}
      </div>
    </ReportSection>
  );
}

function Iv2slsEndogeneitySection({ diag }: { diag: DiagnosticInfo }) {
  if (!diag.iv2sls_endogenous && !diag.iv2sls_hausman) return null;

  return (
    <ReportSection title="Tests of Endogeneity" icon="endogeneity">
      <div className="mb-4">
        <div className="mb-2 px-1">
          <span className="text-[11px] uppercase tracking-wider text-muted-foreground">H0: variables are exogenous</span>
        </div>
        <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
          {diag.iv2sls_endogenous ? (
            <>
              <div className="rounded-lg border border-border bg-muted px-4 py-3 transition-colors hover:border-border">
                <div className="mb-2 font-mono text-[11px] text-muted-foreground">Durbin (score)</div>
                <div className="flex flex-wrap items-baseline gap-x-4 gap-y-1 text-xs">
                  <span className="text-muted-foreground">
                    chi2({diag.iv2sls_endogenous.df}) ={' '}
                    <span className="font-mono text-foreground">{formatNum(diag.iv2sls_endogenous.durbin_stat)}</span>
                  </span>
                  <span className="text-muted-foreground">
                    p ={' '}
                    <span
                      className={`font-mono ${diag.iv2sls_endogenous.durbin_p_value < 0.05 ? 'text-emerald-400' : 'text-muted-foreground'}`}
                    >
                      {formatNum(diag.iv2sls_endogenous.durbin_p_value)}
                    </span>
                  </span>
                </div>
                <div className="mt-1.5 text-[10px]">
                  {diag.iv2sls_endogenous.durbin_p_value < 0.05 ? (
                    <span className="text-amber-400">拒绝 H0</span>
                  ) : (
                    <span className="text-muted-foreground">不拒绝 H0</span>
                  )}
                </div>
              </div>
              <div className="rounded-lg border border-border bg-muted px-4 py-3 transition-colors hover:border-border">
                <div className="mb-2 font-mono text-[11px] text-muted-foreground">Wu-Hausman</div>
                <div className="flex flex-wrap items-baseline gap-x-4 gap-y-1 text-xs">
                  <span className="text-muted-foreground">
                    F({diag.iv2sls_endogenous.df},{diag.iv2sls_endogenous.wu_df_denom}) ={' '}
                    <span className="font-mono text-foreground">{formatNum(diag.iv2sls_endogenous.wu_stat)}</span>
                  </span>
                  <span className="text-muted-foreground">
                    p ={' '}
                    <span
                      className={`font-mono ${diag.iv2sls_endogenous.wu_p_value < 0.05 ? 'text-emerald-400' : 'text-muted-foreground'}`}
                    >
                      {formatNum(diag.iv2sls_endogenous.wu_p_value)}
                    </span>
                  </span>
                </div>
                <div className="mt-1.5 text-[10px]">
                  {diag.iv2sls_endogenous.wu_p_value < 0.05 ? (
                    <span className="text-amber-400">拒绝 H0</span>
                  ) : (
                    <span className="text-muted-foreground">不拒绝 H0</span>
                  )}
                </div>
              </div>
            </>
          ) : null}
          {diag.iv2sls_hausman ? (
            <div className="rounded-lg border border-border bg-muted px-4 py-3 transition-colors hover:border-border">
              <div className="mb-2 font-mono text-[11px] text-muted-foreground">Hausman (sigmamore)</div>
              <div className="flex flex-wrap items-baseline gap-x-4 gap-y-1 text-xs">
                <span className="text-muted-foreground">
                  chi2({diag.iv2sls_hausman.df}) ={' '}
                  <span className="font-mono text-foreground">{formatNum(diag.iv2sls_hausman.stat)}</span>
                </span>
                <span className="text-muted-foreground">
                  p ={' '}
                  <span
                    className={`font-mono ${diag.iv2sls_hausman.p_value < 0.05 ? 'text-emerald-400' : 'text-muted-foreground'}`}
                  >
                    {formatNum(diag.iv2sls_hausman.p_value)}
                  </span>
                </span>
              </div>
              <div className="mt-1.5 text-[10px]">
                {diag.iv2sls_hausman.p_value < 0.05 ? (
                  <span className="text-amber-400">拒绝 H0</span>
                ) : (
                  <span className="text-muted-foreground">不拒绝 H0</span>
                )}
              </div>
            </div>
          ) : null}
        </div>
        <p className="mt-2 px-1 text-xs text-muted-foreground">
          estat endogenous: Durbin &amp; Wu-Hausman. hausman iv ols, constant sigmamore: traditional Hausman. Significant
          p-value favors IV.
        </p>
      </div>
    </ReportSection>
  );
}

export function IvReportSections({
  variant,
  endogName,
  coefficients,
  diag,
}: {
  variant: IvVariant;
  endogName: string;
  coefficients: Coefficient[];
  diag: DiagnosticInfo;
}) {
  const firstStage = diag.iv2sls_first_stage;

  return (
    <>
      <IvEquationSection endogName={endogName} coefficients={coefficients} firstStage={firstStage} />

      {firstStage && firstStage.length > 0 ? <IvFirstStageResultsSection firstStage={firstStage} /> : null}

      {diag.iv2sls_first_stage_summary ? (
        <IvFirstStageSummarySection
          variant={variant}
          summary={diag.iv2sls_first_stage_summary}
          firstStage={firstStage}
        />
      ) : null}

      {variant === '2sls' ? <Iv2slsOveridSection diag={diag} /> : <IvLimlOveridSection diag={diag} />}

      {variant === '2sls' ? <Iv2slsEndogeneitySection diag={diag} /> : null}
    </>
  );
}
