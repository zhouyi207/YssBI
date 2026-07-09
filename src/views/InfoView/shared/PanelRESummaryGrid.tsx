import { formatNum, InfoRow } from './RegressionShared';
import type { PanelFEInfo } from '@/shared/types/report';
import type { ModelBasicInfo } from '@/shared/types/report';

/** Stata xtreg, re style summary grid — Random-effects GLS or ML regression */
export function PanelRESummaryGrid({
  info,
  panelFe,
}: {
  info: ModelBasicInfo;
  panelFe: PanelFEInfo;
}) {
  const isMle = info.lr_chi2 != null;
  const waldChi2 = info.lr_chi2 ?? info.wald_chi2 ?? info.f_statistic;
  const probWald = info.prob_lr_chi2 ?? info.prob_wald_chi2 ?? info.prob_f_statistic;
  const isCluster = info.covariance_type?.toLowerCase().includes('cluster');

  return (
    <div className="space-y-2 mb-2">
      {/* Header: Random-effects GLS or ML regression */}
      <div className="text-xs text-muted-foreground mb-2">
        {isMle ? 'Random-effects ML regression' : 'Random-effects GLS regression'}
      </div>

      {/* Number of obs, Number of groups */}
      <div className="grid grid-cols-2 gap-px bg-border rounded-lg overflow-hidden border border-border">
        <InfoRow label="Number of obs">{info.num_observation}</InfoRow>
        <InfoRow label="Number of groups">{panelFe.num_groups}</InfoRow>
      </div>

      {/* R-squared: Within, Between, Overall (FGLS only; MLE does not report these) */}
      {panelFe.r2_within != null && panelFe.r2_between != null && panelFe.r2_overall != null && (
        <div className="grid grid-cols-2 gap-px bg-border rounded-lg overflow-hidden border border-border">
          <InfoRow label="R-squared Within">{formatNum(panelFe.r2_within)}</InfoRow>
          <InfoRow label="R-squared Between">{formatNum(panelFe.r2_between)}</InfoRow>
          <InfoRow label="R-squared Overall">{formatNum(panelFe.r2_overall)}</InfoRow>
          <InfoRow label="Adj. R-squared">{formatNum(info.adj_r_squared)}</InfoRow>
        </div>
      )}

      {/* Obs per group */}
      <div className="grid grid-cols-2 gap-px bg-border rounded-lg overflow-hidden border border-border">
        <InfoRow label="Obs per group (min)">{panelFe.obs_per_group.min}</InfoRow>
        <InfoRow label="Obs per group (avg)">{formatNum(panelFe.obs_per_group.avg, 1)}</InfoRow>
        <InfoRow label="Obs per group (max)">{panelFe.obs_per_group.max}</InfoRow>
      </div>

      {/* Wald/LR chi2, Prob > chi2, Log likelihood (MLE), corr(u_i, X) = 0 (assumed) */}
      <div className="grid grid-cols-2 gap-px bg-border rounded-lg overflow-hidden border border-border">
        {isMle && info.log_likelihood != null && (
          <InfoRow label="Log likelihood">{formatNum(info.log_likelihood)}</InfoRow>
        )}
        <InfoRow label={isMle ? `LR chi2(${info.df_model})` : `Wald chi2(${info.df_model})`}>
          {formatNum(waldChi2)}
        </InfoRow>
        <InfoRow label="Prob &gt; chi2">
          <span className={probWald < 0.05 ? 'text-emerald-400' : 'text-muted-foreground'}>
            {formatNum(probWald)}
          </span>
        </InfoRow>
        <div className="bg-card px-4 py-2.5 flex justify-between col-span-2">
          <span className="text-muted-foreground text-xs">corr(u_i, X) = 0 (assumed)</span>
        </div>
        {isMle && (
          <div className="bg-card px-4 py-2.5 flex justify-between col-span-2">
            <span className="text-muted-foreground text-xs">
              u_i ~ N(0, σ²_u), ε_it ~ N(0, σ²_e)
            </span>
          </div>
        )}
        {isCluster && (
          <div className="bg-card px-4 py-2.5 flex justify-between col-span-2">
            <span className="text-muted-foreground text-xs">
              (Std. err. adjusted for {panelFe.num_groups} clusters)
            </span>
          </div>
        )}
        <InfoRow label="Covariance Type">{info.covariance_type}</InfoRow>
      </div>

      {/* sigma_u, sigma_e, rho */}
      <div className="grid grid-cols-2 gap-px bg-border rounded-lg overflow-hidden border border-border">
        <InfoRow label="sigma_u">{formatNum(panelFe.sigma.sigma_u)}</InfoRow>
        <InfoRow label="sigma_e">{formatNum(panelFe.sigma.sigma_e)}</InfoRow>
        <div className="bg-card px-4 py-2.5 flex justify-between col-span-2">
          <span className="text-muted-foreground text-xs">rho</span>
          <span className="text-foreground text-xs font-mono font-medium">
            {formatNum(panelFe.sigma.rho)} (fraction of variance due to u_i)
          </span>
        </div>
      </div>

      {/* theta (RE quasi-demeaning parameter) */}
      {panelFe.theta != null && (
        <div className="grid grid-cols-2 gap-px bg-border rounded-lg overflow-hidden border border-border">
          {Math.abs(panelFe.theta.min - panelFe.theta.max) > 1e-10 ? (
            <>
              <InfoRow label="theta (min)">{formatNum(panelFe.theta.min)}</InfoRow>
              <InfoRow label="theta (avg)">{formatNum(panelFe.theta.avg)}</InfoRow>
              <InfoRow label="theta (max)">{formatNum(panelFe.theta.max)}</InfoRow>
            </>
          ) : (
            <div className="bg-card px-4 py-2.5 flex justify-between col-span-2">
              <span className="text-muted-foreground text-xs">theta</span>
              <span className="text-foreground text-xs font-mono font-medium">
                {formatNum(panelFe.theta.avg)}
              </span>
            </div>
          )}
        </div>
      )}

      {/* MLE: LR test of sigma_u=0 */}
      {isMle && panelFe.chibar2 != null && panelFe.prob_chibar2 != null && (
        <div className="grid grid-cols-2 gap-px bg-border rounded-lg overflow-hidden border border-border">
          <InfoRow label="LR test of sigma_u=0: chibar2(01)">{formatNum(panelFe.chibar2)}</InfoRow>
          <InfoRow label="Prob &gt;= chibar2">
            <span className={panelFe.prob_chibar2 < 0.05 ? 'text-emerald-400' : 'text-muted-foreground'}>
              {formatNum(panelFe.prob_chibar2)}
            </span>
          </InfoRow>
        </div>
      )}
    </div>
  );
}
