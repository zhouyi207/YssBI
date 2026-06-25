import React, { Suspense, useMemo } from 'react';
import {
  SectionHeader,
  formatNum,
  CoefficientsBlock,
  VARStableChart,
  VarModelTable,
  VarModelRow,
  VarModelCell,
  VarEigenvalueTable,
  InfoStatsTable,
  infoVarHeadClass,
} from './shared';
import { TableBody, TableHead, TableHeader, TableRow } from '@/components/ui/table';
import type { Coefficient, VARSummaryResultData } from './shared/types';

const VARFormulaBlock = React.lazy(() => import('./VARFormulaBlock'));

export type { VARSummaryResultData } from './shared/types';

function varCoeffsToOLSFormat(coefficients: VARSummaryResultData['coefficients']): Coefficient[] {
  const eqOrder = [...new Set(coefficients.map((x) => x.eq_name))];
  const mapped = coefficients.map((c, idx) => ({
    variable: c.variable,
    category: c.eq_name,
    coef: c.coef,
    std_err: c.std_err,
    t_value: c.z_value,
    p_value: c.p_value,
    'confidence_interval_0.025': c.ci_lower,
    'confidence_interval_0.975': c.ci_upper,
    is_significant: c.p_value < 0.05,
    _sortKey: c.variable === 'const' ? 0 : 1,
    _eqOrder: eqOrder.indexOf(c.eq_name),
    _idx: idx,
  }));
  mapped.sort((a, b) => {
    if (a._eqOrder !== b._eqOrder) return a._eqOrder - b._eqOrder;
    if (a._sortKey !== b._sortKey) return a._sortKey - b._sortKey;
    return a._idx - b._idx;
  });
  return mapped.map(({ _sortKey, _eqOrder, _idx, ...rest }) => rest as Coefficient);
}

export const VARComponent: React.FC<{ data: VARSummaryResultData }> = ({ data }) => {
  const {
    var_names,
    num_observation,
    complete_sample_rows,
    var_max_lag,
    log_likelihood,
    aic,
    fpe,
    hqic,
    sbic,
    equations,
    coefficients,
    oirf,
    fevd,
    varwle,
    varlmar,
    varstable,
    vargranger,
  } = data;

  const coeffsForTable = useMemo(() => varCoeffsToOLSFormat(coefficients), [coefficients]);
  const varstableSorted = useMemo(
    () => (varstable ? [...varstable].sort((a, b) => b.modulus - a.modulus) : []),
    [varstable]
  );

  return (
    <div className="p-6 max-w-[900px] mx-auto">
      {/* Title */}
      <div className="mb-6">
        <h1 className="text-xl font-bold text-foreground mb-2">{data.title}</h1>
        <div className="flex items-center gap-3 flex-wrap">
          <span className="text-xs text-muted-foreground leading-relaxed">
            Variables: {var_names.join(', ')}
            {complete_sample_rows != null && var_max_lag != null ? (
              <>
                {' '}
                · T={complete_sample_rows}（时间轴对齐行数）· p={var_max_lag} · n={num_observation}
                （Stata Number of obs；仅内生 listwise 时 n = T − p；有外生 DataFrame 时与 Stata var ex() 相同，仅当期
                exog[t] 须有效）
              </>
            ) : (
              <> · n={num_observation}</>
            )}
          </span>
        </div>
      </div>

      {/* Equation */}
      <SectionHeader
        title="Equation"
        icon={
          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4.745 3A23.933 23.933 0 003 12c0 3.183.62 6.22 1.745 9M19.5 3c.967 2.78 1.5 5.817 1.5 9s-.533 6.22-1.5 9M8.25 8.885l1.444-.89a.75.75 0 011.105.402l2.402 7.206a.75.75 0 001.104.401l1.445-.889" />
          </svg>
        }
      />
      <Suspense fallback={<div className="rounded-lg border border-border bg-card h-24 animate-pulse" />}>
        <VARFormulaBlock varNames={var_names} coefficients={coefficients} />
      </Suspense>

      {/* Model Summary */}
      <SectionHeader
        title="Model Summary"
        icon={
          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 17v-2m3 2v-4m3 4v-6m2 10H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
          </svg>
        }
      />
      <div className="grid grid-cols-2 gap-px bg-border rounded-lg overflow-hidden border border-border mb-6">
        {complete_sample_rows != null && var_max_lag != null && (
          <div className="bg-card px-4 py-2.5 flex justify-between col-span-2 border-b border-border">
            <span className="text-muted-foreground text-xs shrink-0">Observations</span>
            <span className="text-foreground text-xs font-mono text-right">
              T = {complete_sample_rows}, p = {var_max_lag}, n = {num_observation}{' '}
              <span className="text-muted-foreground font-sans">
                （无缺失外生时 n = T − p；首期外生缺失不减少 n）
              </span>
            </span>
          </div>
        )}
        <div className="bg-card px-4 py-2.5 flex justify-between">
          <span className="text-muted-foreground text-xs">Log likelihood</span>
          <span className="text-foreground text-xs font-mono font-medium">{formatNum(log_likelihood)}</span>
        </div>
        <div className="bg-card px-4 py-2.5 flex justify-between">
          <span className="text-muted-foreground text-xs">AIC</span>
          <span className="text-foreground text-xs font-mono font-medium">{formatNum(aic)}</span>
        </div>
        <div className="bg-card px-4 py-2.5 flex justify-between">
          <span className="text-muted-foreground text-xs">FPE</span>
          <span className="text-foreground text-xs font-mono font-medium">{formatNum(fpe)}</span>
        </div>
        <div className="bg-card px-4 py-2.5 flex justify-between">
          <span className="text-muted-foreground text-xs">HQIC</span>
          <span className="text-foreground text-xs font-mono font-medium">{formatNum(hqic)}</span>
        </div>
        <div className="bg-card px-4 py-2.5 flex justify-between">
          <span className="text-muted-foreground text-xs">SBIC</span>
          <span className="text-foreground text-xs font-mono font-medium">{formatNum(sbic)}</span>
        </div>
        <div className="bg-card px-4 py-2.5 flex justify-between">
          <span className="text-muted-foreground text-xs">Det(Sigma_ml)</span>
          <span className="text-foreground text-xs font-mono font-medium">{formatNum(data.det_sigma_ml)}</span>
        </div>
      </div>

      {/* Equation Summary */}
      <SectionHeader
        title="Equation Summary"
        icon={
          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M3 10h18M3 14h18M3 6h18M3 18h18" />
          </svg>
        }
      />
      <VarModelTable className="mb-6" columns={['Equation', 'Parms', 'RMSE', 'R-sq', 'chi2', 'P>chi2']}>
        {equations.map((eq, i) => (
          <VarModelRow key={i}>
            <VarModelCell>{eq.eq_name}</VarModelCell>
            <VarModelCell>{eq.parms}</VarModelCell>
            <VarModelCell>{formatNum(eq.rmse)}</VarModelCell>
            <VarModelCell>{formatNum(eq.r_sq)}</VarModelCell>
            <VarModelCell>{formatNum(eq.chi2)}</VarModelCell>
            <VarModelCell>{formatNum(eq.p_chi2)}</VarModelCell>
          </VarModelRow>
        ))}
      </VarModelTable>

      {/* Coefficients */}
      <CoefficientsBlock
        coefficients={coeffsForTable}
        hasCategorical={true}
        useZStat={true}
        categoryLabel="Equation"
      />

      {/* Wald lag-exclusion (varwle) */}
      {varwle && varwle.length > 0 && (() => {
        const byEq = varwle.reduce<Record<string, typeof varwle>>((acc, row) => {
          (acc[row.eq_name] ??= []).push(row);
          return acc;
        }, {});
        const eqOrder = [...new Set(varwle.map((r) => r.eq_name))];
        return (
          <>
            <SectionHeader
              title="Wald lag-exclusion statistics (varwle)"
              icon={
                <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2M9 5a2 2 0 002 2h2a2 2 0 002-2M9 5a2 2 0 012-2h2a2 2 0 012 2m-6 9l2 2 4-4" />
                </svg>
              }
            />
            <div className="space-y-4 mb-6">
              {eqOrder.map((eqName) => {
                const rows = byEq[eqName] ?? [];
                return (
                  <div key={eqName} className="rounded-lg border border-border bg-muted overflow-hidden">
                    <div className="px-4 py-2.5 text-sm font-medium text-foreground border-b border-border">
                      Equation: {eqName}
                    </div>
                    <VarModelTable columns={['lag', 'chi2', 'df', 'Prob > chi2']}>
                      {rows.map((row, i) => (
                        <VarModelRow key={i}>
                          <VarModelCell>{row.lag}</VarModelCell>
                          <VarModelCell>{formatNum(row.chi2)}</VarModelCell>
                          <VarModelCell>{row.df}</VarModelCell>
                          <VarModelCell>{formatNum(row.p_value)}</VarModelCell>
                        </VarModelRow>
                      ))}
                    </VarModelTable>
                  </div>
                );
              })}
            </div>
          </>
        );
      })()}

      {/* Eigenvalue stability condition (varstable, graph) */}
      {varstableSorted.length > 0 && (
        <>
          <SectionHeader
            title="Eigenvalue stability condition"
            icon={
              <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002-2V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 01-2 2h-2a2 2 0 01-2-2z" />
              </svg>
            }
          />
          <div className="grid grid-cols-[auto_1fr] gap-4 mb-6 items-stretch min-h-[360px]">
            <div className="flex flex-col h-full rounded-lg border border-border bg-muted overflow-hidden">
              <div className="flex-1 min-h-0 flex flex-col">
                <VarEigenvalueTable rows={varstableSorted} />
                <div className="flex-1 min-h-0 bg-muted" />
              </div>
              <div className="px-4 py-2 text-[11px] text-muted-foreground border-t border-border shrink-0">
                {varstableSorted.some((r) => r.modulus >= 1)
                  ? 'At least one eigenvalue is at least 1.0. VAR does not satisfy stability condition.'
                  : 'All the eigenvalues lie inside the unit circle.'}
              </div>
            </div>
            <div className="min-w-[240px] min-h-0 flex">
              <VARStableChart data={varstableSorted} />
            </div>
          </div>
        </>
      )}

      {/* Granger causality Wald tests (vargranger) */}
      {vargranger && vargranger.length > 0 && (
        <>
          <SectionHeader
            title="Granger causality Wald tests"
            icon={
              <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M13 7h8m0 0v8m0-8l-8 8-4-4-6 6" />
              </svg>
            }
          />
          <VarModelTable className="mb-6" columns={['Equation', 'Excluded', 'chi2', 'df', 'Prob > chi2']}>
            {vargranger.map((row, i) => (
              <VarModelRow key={i} className={row.excluded === 'ALL' ? 'border-b-2 border-border' : undefined}>
                <VarModelCell>{row.eq_name}</VarModelCell>
                <VarModelCell>{row.excluded}</VarModelCell>
                <VarModelCell>{formatNum(row.chi2)}</VarModelCell>
                <VarModelCell>{row.df}</VarModelCell>
                <VarModelCell>{formatNum(row.p_value)}</VarModelCell>
              </VarModelRow>
            ))}
          </VarModelTable>
        </>
      )}

      {/* LM test for residual autocorrelation (varlmar) */}
      {varlmar && varlmar.length > 0 && (
        <>
          <SectionHeader
            title="Lagrange-multiplier test (varlmar)"
            icon={
              <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002 2h2a2 2 0 002-2m0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 01-2 2h-2a2 2 0 01-2-2z" />
              </svg>
            }
          />
          <VarModelTable
            className="mb-6"
            columns={['lag', 'chi2', 'df', 'Prob > chi2']}
            footer={
              <div className="border-t border-border px-4 py-2 text-[11px] text-muted-foreground">
                H0: no autocorrelation at lag order
              </div>
            }
          >
            {varlmar.map((row, i) => (
              <VarModelRow key={i}>
                <VarModelCell>{row.lag}</VarModelCell>
                <VarModelCell>{formatNum(row.chi2)}</VarModelCell>
                <VarModelCell>{row.df}</VarModelCell>
                <VarModelCell>{formatNum(row.p_value)}</VarModelCell>
              </VarModelRow>
            ))}
          </VarModelTable>
        </>
      )}

      {/* Orthogonalized IRF */}
      {oirf && oirf.length > 0 && var_names && var_names.length > 0 && (
        <>
          <SectionHeader
            title="Orthogonalized IRF"
            icon={
              <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M7 12l3-3 3 3 4-4M8 21l4-4 4 4M3 4h18M4 4h16v12a1 1 0 01-1 1H5a1 1 0 01-1-1V4z" />
              </svg>
            }
          />
          <InfoStatsTable className="mb-6 overflow-x-auto bg-muted" tableClassName="min-w-[400px] text-left text-sm">
            <TableHeader>
              <TableRow className="border-b border-border hover:bg-transparent">
                <TableHead className={infoVarHeadClass}>Step</TableHead>
                {var_names.flatMap((imp) =>
                  var_names.map((resp) => (
                    <TableHead key={`${imp}-${resp}`} className={infoVarHeadClass}>
                      {imp}→{resp}
                    </TableHead>
                  )),
                )}
              </TableRow>
            </TableHeader>
            <TableBody>
              {oirf.map((stepData, s) => (
                <VarModelRow key={s}>
                  <VarModelCell>{s}</VarModelCell>
                  {var_names.flatMap((_, impIdx) =>
                    var_names.map((_, respIdx) => (
                      <VarModelCell key={`${impIdx}-${respIdx}`}>{formatNum(stepData[respIdx]?.[impIdx] ?? 0)}</VarModelCell>
                    )),
                  )}
                </VarModelRow>
              ))}
            </TableBody>
          </InfoStatsTable>
        </>
      )}

      {/* FEVD */}
      {fevd && fevd.length > 0 && var_names && var_names.length > 0 && (
        <>
        <SectionHeader
          title="Forecast-error variance decomposition"
          icon={
            <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002 2h2a2 2 0 002-2m0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 01-2 2h-2a2 2 0 01-2-2z" />
            </svg>
          }
        />
        <InfoStatsTable className="mb-6 overflow-x-auto bg-muted" tableClassName="min-w-[400px] text-left text-sm">
          <TableHeader>
            <TableRow className="border-b border-border hover:bg-transparent">
              <TableHead className={infoVarHeadClass}>step</TableHead>
              {var_names.flatMap((imp) =>
                var_names.map((resp) => (
                  <TableHead key={`${imp}-${resp}`} className={infoVarHeadClass}>
                    {imp}→{resp}
                  </TableHead>
                )),
              )}
            </TableRow>
          </TableHeader>
          <TableBody>
            {fevd.map((stepData, s) => (
              <VarModelRow key={s}>
                <VarModelCell>{s}</VarModelCell>
                {var_names.flatMap((_, impIdx) =>
                  var_names.map((_, respIdx) => (
                    <VarModelCell key={`${impIdx}-${respIdx}`}>{formatNum(stepData[respIdx]?.[impIdx] ?? 0)}</VarModelCell>
                  )),
                )}
              </VarModelRow>
            ))}
          </TableBody>
        </InfoStatsTable>
        </>
      )}
    </div>
  );
};
