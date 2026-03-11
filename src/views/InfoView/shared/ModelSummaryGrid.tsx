import React from 'react';
import { formatNum, InfoRow } from './RegressionShared';
import type { ModelBasicInfo } from './types';

export function ModelSummaryGrid({
  info,
  executionTimeMs,
}: {
  info: ModelBasicInfo;
  executionTimeMs?: number;
}) {
  return (
    <div className="grid grid-cols-2 gap-px bg-gray-800/50 rounded-lg overflow-hidden border border-gray-800/50 mb-2">
      <InfoRow label="Model">{info.model_type}</InfoRow>
      <InfoRow label="Method">{info.method}</InfoRow>
      <InfoRow label="R-squared">{info.r_squared.toFixed(4)}</InfoRow>
      <InfoRow label="Adj. R-squared">{info.adj_r_squared.toFixed(4)}</InfoRow>
      {info.wald_chi2 != null ? (
        <>
          <InfoRow label={`Wald chi2(${info.df_model})`}>{formatNum(info.wald_chi2)}</InfoRow>
          <InfoRow label="Prob &gt; chi2">
            <span className={(info.prob_wald_chi2 ?? 1) < 0.05 ? 'text-emerald-400' : 'text-gray-400'}>
              {formatNum(info.prob_wald_chi2 ?? 0)}
            </span>
          </InfoRow>
        </>
      ) : (
        <>
          <InfoRow label="F-statistic">{formatNum(info.f_statistic)}</InfoRow>
          <InfoRow label="Prob (F-statistic)">
            <span className={info.prob_f_statistic < 0.05 ? 'text-emerald-400' : 'text-gray-400'}>
              {formatNum(info.prob_f_statistic)}
            </span>
          </InfoRow>
        </>
      )}
      <InfoRow label="No. Observations">{info.num_observation}</InfoRow>
      <InfoRow label="Covariance Type">{info.covariance_type}</InfoRow>
      <InfoRow label="Df Model">{info.df_model}</InfoRow>
      <InfoRow label="Df Residual">{info.df_residual}</InfoRow>
      {info.aic != null && <InfoRow label="AIC">{formatNum(info.aic)}</InfoRow>}
      {info.bic != null && <InfoRow label="BIC">{formatNum(info.bic)}</InfoRow>}
      <div className="bg-[#13151a] px-4 py-2.5 flex justify-between col-span-2">
        <span className="text-gray-500 text-xs">Df Total</span>
        <span className="text-white text-xs font-mono font-medium">{info.df_total}</span>
      </div>
      {executionTimeMs != null && (
        <div className="bg-[#13151a] px-4 py-2.5 flex justify-between col-span-2 border-t border-gray-800/30">
          <span className="text-gray-500 text-xs">后端计算耗时</span>
          <span className="text-[var(--accent-color)] text-xs font-mono font-medium">{executionTimeMs} ms</span>
        </div>
      )}
    </div>
  );
}
