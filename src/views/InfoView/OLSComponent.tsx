import React, { useMemo } from 'react';

interface ModelBasicInfo {
  model_type: string;
  method: string;
  num_observation: number;
  r_squared: number;
  adj_r_squared: number;
  f_statistic: number;
  prob_f_statistic: number;
  df_model: number;
  df_residual: number;
  df_total: number;
  ss_model: number;
  ss_residual: number;
  ss_total: number;
  ms_model: number;
  ms_residual: number;
  ms_total: number;
  covariance_type: string;
}

interface Coefficient {
  variable: string;
  category?: string;
  coef: number;
  std_err: number;
  t_value: number;
  p_value: number;
  'confidence_interval_0.025': number;
  'confidence_interval_0.975': number;
  is_significant: boolean;
}

interface DiagnosticInfo {
  cond_no: number;
}

export interface OLSResultData {
  title: string;
  model_basic_info: ModelBasicInfo;
  coefficients: Coefficient[];
  diagnostic_info: DiagnosticInfo;
}

function formatNum(value: number, decimals = 4): string {
  if (Math.abs(value) < 0.0001 && value !== 0) {
    return value.toExponential(3);
  }
  return value.toFixed(decimals);
}

function SignificanceStars({ pValue }: { pValue: number }) {
  if (pValue < 0.001) return <span className="text-yellow-400 font-bold ml-1">***</span>;
  if (pValue < 0.01) return <span className="text-yellow-400 font-bold ml-1">**</span>;
  if (pValue < 0.05) return <span className="text-yellow-400 font-bold ml-1">*</span>;
  if (pValue < 0.1) return <span className="text-gray-500 ml-1">.</span>;
  return null;
}

function RSquaredBadge({ value }: { value: number }) {
  let color = 'bg-red-500/20 text-red-400 border-red-500/30';
  if (value >= 0.7) color = 'bg-emerald-500/20 text-emerald-400 border-emerald-500/30';
  else if (value >= 0.4) color = 'bg-yellow-500/20 text-yellow-400 border-yellow-500/30';

  return (
    <span className={`inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-semibold border ${color}`}>
      R² = {value.toFixed(3)}
    </span>
  );
}

function StatCard({ label, value, sub }: { label: string; value: string | number; sub?: string }) {
  return (
    <div className="bg-[#1a1d23] rounded-lg px-4 py-3 border border-gray-800/50">
      <div className="text-[11px] text-gray-500 uppercase tracking-wider mb-1">{label}</div>
      <div className="text-white font-mono text-sm font-medium">{value}</div>
      {sub && <div className="text-[10px] text-gray-600 mt-0.5">{sub}</div>}
    </div>
  );
}

function SectionHeader({ title, icon }: { title: string; icon: React.ReactNode }) {
  return (
    <div className="flex items-center gap-2 mb-3 mt-6 first:mt-0">
      <div className="text-[var(--accent-color)]">{icon}</div>
      <h3 className="text-sm font-semibold text-gray-300 uppercase tracking-wider">{title}</h3>
      <div className="flex-1 h-px bg-gray-800 ml-2"></div>
    </div>
  );
}

function InfoRow({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="bg-[#13151a] px-4 py-2.5 flex justify-between">
      <span className="text-gray-500 text-xs">{label}</span>
      <span className="text-white text-xs font-mono font-medium">{children}</span>
    </div>
  );
}

export const OLSComponent: React.FC<{ data: OLSResultData }> = ({ data }) => {
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
          <RSquaredBadge value={info.r_squared} />
          <span className="text-xs text-gray-500">
            {info.method} &middot; n={info.num_observation}
          </span>
        </div>
      </div>

      {/* Model Info Section */}
      <SectionHeader
        title="Model Summary"
        icon={
          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 17v-2m3 2v-4m3 4v-6m2 10H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
          </svg>
        }
      />

      <div className="grid grid-cols-2 gap-px bg-gray-800/50 rounded-lg overflow-hidden border border-gray-800/50 mb-2">
        <InfoRow label="Model">{info.model_type}</InfoRow>
        <InfoRow label="Method">{info.method}</InfoRow>
        <InfoRow label="R-squared">{info.r_squared.toFixed(4)}</InfoRow>
        <InfoRow label="Adj. R-squared">{info.adj_r_squared.toFixed(4)}</InfoRow>
        <InfoRow label="F-statistic">{formatNum(info.f_statistic)}</InfoRow>
        <InfoRow label="Prob (F-statistic)">
          <span className={info.prob_f_statistic < 0.05 ? 'text-emerald-400' : 'text-gray-400'}>
            {formatNum(info.prob_f_statistic)}
          </span>
        </InfoRow>
        <InfoRow label="No. Observations">{info.num_observation}</InfoRow>
        <InfoRow label="Covariance Type">{info.covariance_type}</InfoRow>
        <InfoRow label="Df Model">{info.df_model}</InfoRow>
        <InfoRow label="Df Residual">{info.df_residual}</InfoRow>
        <div className="bg-[#13151a] px-4 py-2.5 flex justify-between col-span-2">
          <span className="text-gray-500 text-xs">Df Total</span>
          <span className="text-white text-xs font-mono font-medium">{info.df_total}</span>
        </div>
      </div>

      {/* Sum of Squares & Mean Squares */}
      <SectionHeader
        title="ANOVA"
        icon={
          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M3 10h18M3 14h18M3 6h18M3 18h18" />
          </svg>
        }
      />

      <div className="rounded-lg border border-gray-800/50 overflow-hidden mb-2">
        <table className="w-full text-xs">
          <thead>
            <tr className="bg-[#1a1d23]">
              <th className="text-left px-4 py-2.5 text-gray-500 font-medium uppercase tracking-wider">Source</th>
              <th className="text-right px-3 py-2.5 text-gray-500 font-medium uppercase tracking-wider">SS</th>
              <th className="text-right px-3 py-2.5 text-gray-500 font-medium uppercase tracking-wider">df</th>
              <th className="text-right px-3 py-2.5 text-gray-500 font-medium uppercase tracking-wider">MS</th>
            </tr>
          </thead>
          <tbody>
            <tr className="bg-[#13151a] border-t border-gray-800/30">
              <td className="px-4 py-2.5 font-mono text-white">Model</td>
              <td className="text-right px-3 py-2.5 font-mono text-gray-300">{formatNum(info.ss_model)}</td>
              <td className="text-right px-3 py-2.5 font-mono text-gray-300">{info.df_model}</td>
              <td className="text-right px-3 py-2.5 font-mono text-gray-300">{formatNum(info.ms_model)}</td>
            </tr>
            <tr className="bg-[#15171d] border-t border-gray-800/30">
              <td className="px-4 py-2.5 font-mono text-white">Residual</td>
              <td className="text-right px-3 py-2.5 font-mono text-gray-300">{formatNum(info.ss_residual)}</td>
              <td className="text-right px-3 py-2.5 font-mono text-gray-300">{info.df_residual}</td>
              <td className="text-right px-3 py-2.5 font-mono text-gray-300">{formatNum(info.ms_residual)}</td>
            </tr>
            <tr className="bg-[#13151a] border-t border-gray-800/30">
              <td className="px-4 py-2.5 font-mono text-white font-semibold">Total</td>
              <td className="text-right px-3 py-2.5 font-mono text-white font-semibold">{formatNum(info.ss_total)}</td>
              <td className="text-right px-3 py-2.5 font-mono text-white font-semibold">{info.df_total}</td>
              <td className="text-right px-3 py-2.5 font-mono text-white font-semibold">{formatNum(info.ms_total)}</td>
            </tr>
          </tbody>
        </table>
      </div>

      {/* Coefficients Section */}
      <SectionHeader
        title={`Coefficients (${significantCount}/${coefficients.length} significant)`}
        icon={
          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 7h16M4 12h10M4 17h6" />
          </svg>
        }
      />

      <div className="rounded-lg border border-gray-800/50 overflow-hidden">
        <table className="w-full text-xs">
          <thead>
            <tr className="bg-[#1a1d23]">
              <th className="text-left px-4 py-2.5 text-gray-500 font-medium uppercase tracking-wider">Variable</th>
              {hasCategorical && (
                <th className="text-left px-3 py-2.5 text-gray-500 font-medium uppercase tracking-wider">Category</th>
              )}
              <th className="text-right px-3 py-2.5 text-gray-500 font-medium uppercase tracking-wider">Coef</th>
              <th className="text-right px-3 py-2.5 text-gray-500 font-medium uppercase tracking-wider">Std Err</th>
              <th className="text-right px-3 py-2.5 text-gray-500 font-medium uppercase tracking-wider">t</th>
              <th className="text-right px-3 py-2.5 text-gray-500 font-medium uppercase tracking-wider">P&gt;|t|</th>
              <th className="text-right px-3 py-2.5 text-gray-500 font-medium uppercase tracking-wider">[0.025</th>
              <th className="text-right px-3 py-2.5 text-gray-500 font-medium uppercase tracking-wider">0.975]</th>
            </tr>
          </thead>
          <tbody>
            {coefficients.map((coeff, idx) => (
              <tr
                key={`${coeff.variable}-${coeff.category ?? idx}`}
                className={`
                  border-t border-gray-800/30 transition-colors hover:bg-[#1e2128]
                  ${idx % 2 === 0 ? 'bg-[#13151a]' : 'bg-[#15171d]'}
                `}
              >
                <td className="px-4 py-2.5">
                  <div className="flex items-center gap-2">
                    <div className={`w-1.5 h-1.5 rounded-full ${coeff.is_significant ? 'bg-emerald-400' : 'bg-gray-600'}`} />
                    <span className={`font-mono font-medium ${coeff.is_significant ? 'text-white' : 'text-gray-400'}`}>
                      {coeff.variable}
                    </span>
                  </div>
                </td>
                {hasCategorical && (
                  <td className="px-3 py-2.5">
                    {coeff.category != null ? (
                      <span className="inline-flex items-center px-2 py-0.5 rounded text-[11px] font-mono bg-indigo-500/15 text-indigo-300 border border-indigo-500/25">
                        {coeff.category}
                      </span>
                    ) : (
                      <span className="text-gray-600">—</span>
                    )}
                  </td>
                )}
                <td className="text-right px-3 py-2.5 font-mono text-white">
                  {formatNum(coeff.coef)}
                </td>
                <td className="text-right px-3 py-2.5 font-mono text-gray-400">
                  {formatNum(coeff.std_err)}
                </td>
                <td className="text-right px-3 py-2.5 font-mono text-gray-300">
                  {formatNum(coeff.t_value, 3)}
                </td>
                <td className="text-right px-3 py-2.5 font-mono">
                  <span className={coeff.is_significant ? 'text-emerald-400' : 'text-gray-500'}>
                    {formatNum(coeff.p_value, 3)}
                  </span>
                  <SignificanceStars pValue={coeff.p_value} />
                </td>
                <td className="text-right px-3 py-2.5 font-mono text-gray-500">
                  {formatNum(coeff['confidence_interval_0.025'])}
                </td>
                <td className="text-right px-3 py-2.5 font-mono text-gray-500">
                  {formatNum(coeff['confidence_interval_0.975'])}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      <div className="flex items-center gap-4 mt-2 text-[10px] text-gray-600 px-1">
        <span>Significance: <span className="text-yellow-400">***</span> p&lt;0.001, <span className="text-yellow-400">**</span> p&lt;0.01, <span className="text-yellow-400">*</span> p&lt;0.05, <span className="text-gray-500">.</span> p&lt;0.1</span>
      </div>

      {/* Coefficient Bar Visualization */}
      <SectionHeader
        title="Coefficient Magnitude"
        icon={
          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M16 8v8m-4-5v5m-4-2v2m-2 4h12a2 2 0 002-2V6a2 2 0 00-2-2H6a2 2 0 00-2 2v12a2 2 0 002 2z" />
          </svg>
        }
      />
      <CoeffBarChart coefficients={coefficients} />

      {/* Diagnostic Section */}
      <SectionHeader
        title="Diagnostics"
        icon={
          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z" />
          </svg>
        }
      />

      <div className="grid grid-cols-1 gap-3">
        <StatCard
          label="Condition Number"
          value={formatNum(diag.cond_no)}
          sub={diag.cond_no > 1000 ? 'Possible multicollinearity' : 'Acceptable'}
        />
      </div>
    </div>
  );
};

function CoeffBarChart({ coefficients }: { coefficients: Coefficient[] }) {
  const maxAbs = Math.max(...coefficients.map((c) => Math.abs(c.coef)), 0.001);

  return (
    <div className="rounded-lg border border-gray-800/50 bg-[#13151a] p-4 space-y-2">
      {coefficients.map((coeff, idx) => {
        const pct = (Math.abs(coeff.coef) / maxAbs) * 100;
        const isPositive = coeff.coef >= 0;
        const label = coeff.category != null
          ? `${coeff.variable}[${coeff.category}]`
          : coeff.variable;

        return (
          <div key={`${coeff.variable}-${coeff.category ?? idx}`} className="flex items-center gap-3">
            <span className="text-xs font-mono text-gray-400 w-28 text-right shrink-0 truncate" title={label}>
              {label}
            </span>
            <div className="flex-1 flex items-center h-5">
              <div className="w-1/2 flex justify-end">
                {!isPositive && (
                  <div
                    className={`h-4 rounded-l transition-all ${coeff.is_significant ? 'bg-rose-500/70' : 'bg-rose-500/25'}`}
                    style={{ width: `${pct}%`, minWidth: pct > 0 ? '2px' : '0' }}
                  />
                )}
              </div>
              <div className="w-px h-5 bg-gray-700 shrink-0" />
              <div className="w-1/2 flex justify-start">
                {isPositive && (
                  <div
                    className={`h-4 rounded-r transition-all ${coeff.is_significant ? 'bg-emerald-500/70' : 'bg-emerald-500/25'}`}
                    style={{ width: `${pct}%`, minWidth: pct > 0 ? '2px' : '0' }}
                  />
                )}
              </div>
            </div>
            <span className="text-[10px] font-mono text-gray-500 w-20 text-left shrink-0">
              {formatNum(coeff.coef)}
            </span>
          </div>
        );
      })}
    </div>
  );
}
