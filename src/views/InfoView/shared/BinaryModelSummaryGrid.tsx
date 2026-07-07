import { formatNum, InfoRow } from './RegressionShared';
import type { ModelBasicInfo } from './types';

/** Model summary for binary choice models (Logit, Probit) */
export function BinaryModelSummaryGrid({
  info,
  executionTimeMs,
}: {
  info: ModelBasicInfo;
  executionTimeMs?: number;
}) {
  return (
    <div className="grid grid-cols-2 gap-px bg-border rounded-lg overflow-hidden border border-border mb-2">
      <InfoRow label="Model">{info.model_type}</InfoRow>
      <InfoRow label="Method">{info.method}</InfoRow>
      <InfoRow label="Pseudo R-squared">{info.r_squared.toFixed(4)}</InfoRow>
      <InfoRow label="Pseudo Adj. R-squared">{info.adj_r_squared.toFixed(4)}</InfoRow>
      {info.wald_chi2 != null && (
        <>
          <InfoRow label={`LR chi2(${info.df_model})`}>{formatNum(info.wald_chi2)}</InfoRow>
          <InfoRow label="Prob &gt; chi2">
            <span className={(info.prob_wald_chi2 ?? 1) < 0.05 ? 'text-emerald-400' : 'text-muted-foreground'}>
              {formatNum(info.prob_wald_chi2 ?? 0)}
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
      <div className="bg-card px-4 py-2.5 flex justify-between col-span-2">
        <span className="text-muted-foreground text-xs">Df Total</span>
        <span className="text-foreground text-xs font-mono font-medium">{info.df_total}</span>
      </div>
      {executionTimeMs != null && (
        <div className="bg-card px-4 py-2.5 flex justify-between col-span-2 border-t border-border">
          <span className="text-muted-foreground text-xs">后端计算耗时</span>
          <span className="text-[var(--accent-color)] text-xs font-mono font-medium">{executionTimeMs} ms</span>
        </div>
      )}
    </div>
  );
}
