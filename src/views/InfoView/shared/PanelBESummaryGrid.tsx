import { formatNum, InfoRow } from "./RegressionShared";
import type { PanelFEInfo } from "@/shared/types/report";
import type { ModelBasicInfo } from "@/shared/types/report";

/** Stata xtreg, be style summary grid — Between regression (regression on group means) */
export function PanelBESummaryGrid({
  info,
  panelFe,
  effectType = "entity",
}: {
  info: ModelBasicInfo;
  panelFe: PanelFEInfo;
  effectType?: "entity" | "time";
}) {
  const sdLabel = effectType === "time" ? "sd(λ_t + avg(e_.t))" : "sd(u_i + avg(e_i.))";
  return (
    <div className="space-y-2 mb-2">
      {/* Header: Between regression */}
      <div className="text-xs text-muted-foreground mb-2">
        Between regression (regression on group means)
      </div>

      {/* Number of obs, Number of groups */}
      <div className="grid grid-cols-2 gap-px bg-border rounded-lg overflow-hidden border border-border">
        <InfoRow label="Number of obs">{info.num_observation}</InfoRow>
        <InfoRow label="Number of groups">{panelFe.num_groups}</InfoRow>
      </div>

      {/* R-squared: Within, Between, Overall */}
      <div className="grid grid-cols-2 gap-px bg-border rounded-lg overflow-hidden border border-border">
        {panelFe.r2_within != null && (
          <InfoRow label="R-squared Within">{formatNum(panelFe.r2_within)}</InfoRow>
        )}
        {panelFe.r2_between != null && (
          <InfoRow label="R-squared Between">{formatNum(panelFe.r2_between)}</InfoRow>
        )}
        {panelFe.r2_overall != null && (
          <InfoRow label="R-squared Overall">{formatNum(panelFe.r2_overall)}</InfoRow>
        )}
        <InfoRow label="Adj. R-squared">{formatNum(info.adj_r_squared)}</InfoRow>
      </div>

      {/* Obs per group */}
      <div className="grid grid-cols-2 gap-px bg-border rounded-lg overflow-hidden border border-border">
        <InfoRow label="Obs per group (min)">{panelFe.obs_per_group.min}</InfoRow>
        <InfoRow label="Obs per group (avg)">{formatNum(panelFe.obs_per_group.avg, 1)}</InfoRow>
        <InfoRow label="Obs per group (max)">{panelFe.obs_per_group.max}</InfoRow>
      </div>

      {/* F test and sd(u_i + avg(e_i.)) — Stata layout */}
      <div className="grid grid-cols-2 gap-px bg-border rounded-lg overflow-hidden border border-border">
        <InfoRow label={`F(${info.df_model}, ${info.df_residual})`}>
          {formatNum(info.f_statistic)}
        </InfoRow>
        <InfoRow label="Prob &gt; F">
          <span
            className={info.prob_f_statistic < 0.05 ? "text-emerald-400" : "text-muted-foreground"}
          >
            {formatNum(info.prob_f_statistic)}
          </span>
        </InfoRow>
        <div className="bg-card px-4 py-2.5 flex justify-between col-span-2">
          <span className="text-muted-foreground text-xs">{sdLabel}</span>
          <span className="text-foreground text-xs font-mono font-medium">
            {formatNum(panelFe.sigma.sigma_u)}
          </span>
        </div>
        <InfoRow label="Covariance Type">{info.covariance_type}</InfoRow>
      </div>
    </div>
  );
}
