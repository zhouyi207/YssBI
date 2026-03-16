import React from 'react';
import { formatNum, InfoRow } from './RegressionShared';
import type { PanelFEInfo } from './types';
import type { ModelBasicInfo } from './types';

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
      <div className="text-xs text-gray-400 mb-2">
        {isMle ? 'Random-effects ML regression' : 'Random-effects GLS regression'}
      </div>

      {/* Number of obs, Number of groups */}
      <div className="grid grid-cols-2 gap-px bg-gray-800/50 rounded-lg overflow-hidden border border-gray-800/50">
        <InfoRow label="Number of obs">{info.num_observation}</InfoRow>
        <InfoRow label="Number of groups">{panelFe.num_groups}</InfoRow>
      </div>

      {/* R-squared: Within, Between, Overall (FGLS only; MLE does not report these) */}
      {panelFe.r2_within != null && panelFe.r2_between != null && panelFe.r2_overall != null && (
        <div className="grid grid-cols-2 gap-px bg-gray-800/50 rounded-lg overflow-hidden border border-gray-800/50">
          <InfoRow label="R-squared Within">{panelFe.r2_within.toFixed(4)}</InfoRow>
          <InfoRow label="R-squared Between">{panelFe.r2_between.toFixed(4)}</InfoRow>
          <InfoRow label="R-squared Overall">{panelFe.r2_overall.toFixed(4)}</InfoRow>
          <InfoRow label="Adj. R-squared">{info.adj_r_squared.toFixed(4)}</InfoRow>
        </div>
      )}

      {/* Obs per group */}
      <div className="grid grid-cols-2 gap-px bg-gray-800/50 rounded-lg overflow-hidden border border-gray-800/50">
        <InfoRow label="Obs per group (min)">{panelFe.obs_per_group_min}</InfoRow>
        <InfoRow label="Obs per group (avg)">{panelFe.obs_per_group_avg.toFixed(1)}</InfoRow>
        <InfoRow label="Obs per group (max)">{panelFe.obs_per_group_max}</InfoRow>
      </div>

      {/* Wald/LR chi2, Prob > chi2, Log likelihood (MLE), corr(u_i, X) = 0 (assumed) */}
      <div className="grid grid-cols-2 gap-px bg-gray-800/50 rounded-lg overflow-hidden border border-gray-800/50">
        {isMle && info.log_likelihood != null && (
          <InfoRow label="Log likelihood">{formatNum(info.log_likelihood)}</InfoRow>
        )}
        <InfoRow label={isMle ? `LR chi2(${info.df_model})` : `Wald chi2(${info.df_model})`}>
          {formatNum(waldChi2)}
        </InfoRow>
        <InfoRow label="Prob &gt; chi2">
          <span className={probWald < 0.05 ? 'text-emerald-400' : 'text-gray-400'}>
            {formatNum(probWald)}
          </span>
        </InfoRow>
        <div className="bg-[#13151a] px-4 py-2.5 flex justify-between col-span-2">
          <span className="text-gray-500 text-xs">corr(u_i, X) = 0 (assumed)</span>
        </div>
        {isMle && (
          <div className="bg-[#13151a] px-4 py-2.5 flex justify-between col-span-2">
            <span className="text-gray-500 text-xs">
              u_i ~ N(0, σ²_u), ε_it ~ N(0, σ²_e)
            </span>
          </div>
        )}
        {isCluster && (
          <div className="bg-[#13151a] px-4 py-2.5 flex justify-between col-span-2">
            <span className="text-gray-500 text-xs">
              (Std. err. adjusted for {panelFe.num_groups} clusters)
            </span>
          </div>
        )}
        <InfoRow label="Covariance Type">{info.covariance_type}</InfoRow>
      </div>

      {/* sigma_u, sigma_e, rho */}
      <div className="grid grid-cols-2 gap-px bg-gray-800/50 rounded-lg overflow-hidden border border-gray-800/50">
        <InfoRow label="sigma_u">{formatNum(panelFe.sigma_u)}</InfoRow>
        <InfoRow label="sigma_e">{formatNum(panelFe.sigma_e)}</InfoRow>
        <div className="bg-[#13151a] px-4 py-2.5 flex justify-between col-span-2">
          <span className="text-gray-500 text-xs">rho</span>
          <span className="text-white text-xs font-mono font-medium">
            {formatNum(panelFe.rho)} (fraction of variance due to u_i)
          </span>
        </div>
      </div>

      {/* theta (RE quasi-demeaning parameter) */}
      {panelFe.theta_avg != null && (
        <div className="grid grid-cols-2 gap-px bg-gray-800/50 rounded-lg overflow-hidden border border-gray-800/50">
          {panelFe.theta_min != null && panelFe.theta_max != null &&
           Math.abs(panelFe.theta_min - panelFe.theta_max) > 1e-10 ? (
            <>
              <InfoRow label="theta (min)">{formatNum(panelFe.theta_min)}</InfoRow>
              <InfoRow label="theta (avg)">{formatNum(panelFe.theta_avg)}</InfoRow>
              <InfoRow label="theta (max)">{formatNum(panelFe.theta_max)}</InfoRow>
            </>
          ) : (
            <div className="bg-[#13151a] px-4 py-2.5 flex justify-between col-span-2">
              <span className="text-gray-500 text-xs">theta</span>
              <span className="text-white text-xs font-mono font-medium">
                {formatNum(panelFe.theta_avg)}
              </span>
            </div>
          )}
        </div>
      )}

      {/* MLE: LR test of sigma_u=0 */}
      {isMle && panelFe.chibar2 != null && panelFe.prob_chibar2 != null && (
        <div className="grid grid-cols-2 gap-px bg-gray-800/50 rounded-lg overflow-hidden border border-gray-800/50">
          <InfoRow label="LR test of sigma_u=0: chibar2(01)">{formatNum(panelFe.chibar2)}</InfoRow>
          <InfoRow label="Prob &gt;= chibar2">
            <span className={panelFe.prob_chibar2 < 0.05 ? 'text-emerald-400' : 'text-gray-400'}>
              {formatNum(panelFe.prob_chibar2)}
            </span>
          </InfoRow>
        </div>
      )}
    </div>
  );
}
