import { formatNum, InfoRow } from './RegressionShared';
import type { PanelFEInfo } from './types';
import type { ModelBasicInfo } from './types';

/** Stata xtreg, fe style summary grid */
export function PanelFESummaryGrid({
  info,
  panelFe,
}: {
  info: ModelBasicInfo;
  panelFe: PanelFEInfo;
}) {
  return (
    <div className="space-y-2 mb-2">
      {/* Header block: Number of obs, Number of groups */}
      <div className="grid grid-cols-2 gap-px bg-border rounded-lg overflow-hidden border border-border">
        <InfoRow label="Number of obs">{info.num_observation}</InfoRow>
        <InfoRow label="Number of groups">{panelFe.num_groups}</InfoRow>
      </div>

      {/* R-squared: Within, Between, Overall */}
      <div className="grid grid-cols-2 gap-px bg-border rounded-lg overflow-hidden border border-border">
        {panelFe.r2_within != null && (
          <InfoRow label="R-squared Within">{panelFe.r2_within.toFixed(4)}</InfoRow>
        )}
        {panelFe.r2_between != null && (
          <InfoRow label="R-squared Between">{panelFe.r2_between.toFixed(4)}</InfoRow>
        )}
        {panelFe.r2_overall != null && (
          <InfoRow label="R-squared Overall">{panelFe.r2_overall.toFixed(4)}</InfoRow>
        )}
        <InfoRow label="Adj. R-squared">{info.adj_r_squared.toFixed(4)}</InfoRow>
      </div>

      {/* Obs per group */}
      <div className="grid grid-cols-2 gap-px bg-border rounded-lg overflow-hidden border border-border">
        <InfoRow label="Obs per group (min)">{panelFe.obs_per_group.min}</InfoRow>
        <InfoRow label="Obs per group (avg)">{panelFe.obs_per_group.avg.toFixed(1)}</InfoRow>
        <InfoRow label="Obs per group (max)">{panelFe.obs_per_group.max}</InfoRow>
      </div>

      {/* F test */}
      <div className="grid grid-cols-2 gap-px bg-border rounded-lg overflow-hidden border border-border">
        <InfoRow label={`F(${info.df_model}, ${info.df_residual})`}>{formatNum(info.f_statistic)}</InfoRow>
        <InfoRow label="Prob &gt; F">
          <span className={info.prob_f_statistic < 0.05 ? 'text-emerald-400' : 'text-muted-foreground'}>
            {formatNum(info.prob_f_statistic)}
          </span>
        </InfoRow>
      </div>

      {/* corr(u_i, Xb) */}
      <div className="grid grid-cols-2 gap-px bg-border rounded-lg overflow-hidden border border-border">
        <InfoRow label="corr(u_i, Xb)">{formatNum(panelFe.corr_u_i_Xb)}</InfoRow>
        <InfoRow label="Covariance Type">{info.covariance_type}</InfoRow>
      </div>

      {/* sigma_u, sigma_e, rho */}
      <div className="grid grid-cols-2 gap-px bg-border rounded-lg overflow-hidden border border-border">
        <InfoRow label="sigma_u">{formatNum(panelFe.sigma.sigma_u)}</InfoRow>
        <InfoRow label="sigma_e">{formatNum(panelFe.sigma.sigma_e)}</InfoRow>
        <div className="bg-card px-4 py-2.5 flex justify-between col-span-2">
          <span className="text-muted-foreground text-xs">rho</span>
          <span className="text-foreground text-xs font-mono font-medium">
            {formatNum(panelFe.sigma.rho)}{' '}
            <span className="text-muted-foreground font-normal">(fraction of variance due to u_i)</span>
          </span>
        </div>
      </div>
    </div>
  );
}
