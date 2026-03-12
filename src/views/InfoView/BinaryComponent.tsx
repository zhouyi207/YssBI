import React, { Suspense, useMemo } from 'react';
import {
  SectionHeader,
  BinaryModelSummaryGrid,
  ClassificationTableBlock,
  CoefficientTable,
  CoeffBarChart,
  HypothesisTestBlock,
  MarginsBlock,
} from './shared';
import type { OLSResultData } from './shared/types';

const BinaryFormulaBlock = React.lazy(() => import('./BinaryFormulaBlock'));
const Scatter = React.lazy(() => import('@/views/PlotView/Scatter'));

export type { OLSResultData };

/** Binary choice model component (Logit, Probit) */
export const BinaryComponent: React.FC<{ data: OLSResultData }> = ({ data }) => {
  const { model_basic_info: info, coefficients, diagnostic_info: diag } = data;

  const significantCount = useMemo(
    () => coefficients.filter((c) => c.is_significant).length,
    [coefficients]
  );

  const hasCategorical = useMemo(
    () => coefficients.some((c) => c.category != null),
    [coefficients]
  );

  return (
    <div className="p-6 max-w-[900px] mx-auto">
      {/* Title */}
      <div className="mb-6">
        <h1 className="text-xl font-bold text-white mb-2">{data.title}</h1>
        <div className="flex items-center gap-3 flex-wrap">
          <span className="inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-semibold border bg-emerald-500/20 text-emerald-400 border-emerald-500/30">
            Pseudo R² = {info.r_squared.toFixed(3)}
          </span>
          <span className="text-xs text-gray-500">
            {info.method} &middot; n={info.num_observation}
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
      <Suspense fallback={<div className="rounded-lg border border-gray-800/50 bg-[#13151a] h-24 animate-pulse" />}>
        <BinaryFormulaBlock
          modelType={info.model_type === 'Probit' ? 'Probit' : 'Logit'}
          endogName={data.endog_name || 'y'}
          coefficients={coefficients}
        />
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
      <BinaryModelSummaryGrid info={info} executionTimeMs={data.executionTimeMs} />

      {/* Classification Table (estat clas) */}
      {diag.classification_table && (
        <>
          <SectionHeader
            title="Classification Table"
            icon={
              <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2M9 5a2 2 0 002 2h2a2 2 0 002-2M9 5a2 2 0 012-2h2a2 2 0 012 2m-3 7h3m-3 4h3m-6-4h.01M9 16h.01" />
              </svg>
            }
          />
          <ClassificationTableBlock data={diag.classification_table} />
        </>
      )}

      {/* Coefficients */}
      <SectionHeader
        title={`Coefficients (${significantCount}/${coefficients.length} significant)`}
        icon={
          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 7h16M4 12h10M4 17h6" />
          </svg>
        }
      />
      <CoefficientTable
        coefficients={coefficients}
        hasCategorical={hasCategorical}
        useZStat
        showOddsRatio={info.model_type === 'Logit'}
      />

      {/* Margins (Stata margins) */}
      <MarginsBlock data={data} />

      {/* Hypothesis Test */}
      <HypothesisTestBlock data={data} />

      {/* Coefficient Bar */}
      <SectionHeader
        title="Coefficient Magnitude"
        icon={
          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M16 8v8m-4-5v5m-4-2v2m-2 4h12a2 2 0 002-2V6a2 2 0 00-2-2H6a2 2 0 00-2 2v12a2 2 0 002 2z" />
          </svg>
        }
      />
      <CoeffBarChart coefficients={coefficients} />

      {/* Fitted vs Residuals (deviance residuals) */}
      {diag.fitted_values && diag.residuals && diag.fitted_values.length > 0 && (
        <>
          <SectionHeader
            title="Residuals vs Fitted (Probabilities)"
            icon={
              <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M3 10h18M3 14h18M3 6h18M3 18h18" />
              </svg>
            }
          />
          <Suspense fallback={<div className="rounded-lg border border-gray-800/50 bg-[#13151a] h-[280px] animate-pulse" />}>
            <Scatter
              data={diag.fitted_values.map((x, i) => ({ x, y: (diag.residuals ?? [])[i] ?? 0 }))}
              xLabel="Fitted (P)"
              yLabel="Residual (y - P)"
              height={280}
              symmetricY
              zeroLine
            />
          </Suspense>
        </>
      )}
    </div>
  );
};
