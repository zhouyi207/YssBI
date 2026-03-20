import React from 'react';
import { SectionHeader, formatNum } from './shared';
import type { VecRankResultData } from './shared/types';

export type { VecRankResultData } from './shared/types';

function fmt(v: number | null | undefined, d = 4): string {
  if (v == null || Number.isNaN(v)) return '—';
  return formatNum(v, d);
}

export const VecRankComponent: React.FC<{ data: VecRankResultData }> = ({ data }) => {
  const {
    title,
    var_names,
    num_observation,
    n_lags,
    trend_spec,
    show_max_eigen,
    selected_rank_trace_95,
    selected_rank_trace_99,
    selected_rank_max_95,
    selected_rank_max_99,
    rows,
    note,
  } = data;

  const trendLabel =
    trend_spec === 'none'
      ? 'none'
      : trend_spec === 'constant'
        ? 'constant'
        : trend_spec === 'trend'
          ? 'trend'
          : trend_spec;

  return (
    <div className="p-6 max-w-[1100px] mx-auto">
      <div className="mb-6">
        <h1 className="text-xl font-bold text-white mb-2">{title}</h1>
        <p className="text-xs text-gray-500 leading-relaxed">
          Variables: {var_names.join(', ')} · Trend: {trendLabel} · Number of obs = {num_observation} · Lags = {n_lags}
          <br />
          Trace @5% / 1% selected rank: {selected_rank_trace_95} / {selected_rank_trace_99} · Max eigen @5% / 1%:{' '}
          {selected_rank_max_95} / {selected_rank_max_99}
        </p>
      </div>

      <SectionHeader
        title="Johansen tests"
        icon={
          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M7 12l3-3 3 3 4-4M8 21l4-4 4 4M3 4h18M4 4h16v12a1 1 0 01-1 1H5a1 1 0 01-1-1V4z" />
          </svg>
        }
      />

      <div className="rounded-lg border border-gray-800/50 bg-[#13151a] overflow-x-auto mb-6">
        <table className="w-full text-xs font-mono text-gray-300">
          <thead>
            <tr className="border-b border-gray-800 text-gray-500">
              <th className="px-2 py-2 text-left">rank</th>
              <th className="px-2 py-2 text-right">LL</th>
              <th className="px-2 py-2 text-right">Eigen</th>
              <th className="px-2 py-2 text-right">Trace</th>
              <th className="px-2 py-2 text-right">cv 10%</th>
              <th className="px-2 py-2 text-right">cv 5%</th>
              <th className="px-2 py-2 text-right">cv 1%</th>
              {show_max_eigen && (
                <>
                  <th className="px-2 py-2 text-right border-l border-gray-800/60">λ_max</th>
                  <th className="px-2 py-2 text-right">cv 10%</th>
                  <th className="px-2 py-2 text-right">cv 5%</th>
                  <th className="px-2 py-2 text-right">cv 1%</th>
                </>
              )}
            </tr>
          </thead>
          <tbody>
            {rows.map((r) => {
              const starTrace = r.rank === selected_rank_trace_95 && r.trace_statistic != null;
              const starMax = show_max_eigen && r.rank === selected_rank_max_95 && r.max_eigenvalue_statistic != null;
              return (
                <tr key={r.rank} className="border-b border-gray-800/40 hover:bg-white/[0.02]">
                  <td className="px-2 py-1.5 text-left text-gray-400">{r.rank}</td>
                  <td className="px-2 py-1.5 text-right">{fmt(r.log_likelihood)}</td>
                  <td className="px-2 py-1.5 text-right">{fmt(r.eigenvalue)}</td>
                  <td className="px-2 py-1.5 text-right">
                    {starTrace ? (
                      <span className="text-emerald-400 font-medium">
                        {fmt(r.trace_statistic)} <span className="text-gray-500">*</span>
                      </span>
                    ) : (
                      fmt(r.trace_statistic)
                    )}
                  </td>
                  <td className="px-2 py-1.5 text-right text-gray-500">{fmt(r.trace_crit_10pct)}</td>
                  <td className="px-2 py-1.5 text-right text-gray-500">{fmt(r.trace_crit_5pct)}</td>
                  <td className="px-2 py-1.5 text-right text-gray-500">{fmt(r.trace_crit_1pct)}</td>
                  {show_max_eigen && (
                    <>
                      <td className="px-2 py-1.5 text-right border-l border-gray-800/60">
                        {starMax ? (
                          <span className="text-emerald-400 font-medium">
                            {fmt(r.max_eigenvalue_statistic)} <span className="text-gray-500">*</span>
                          </span>
                        ) : (
                          fmt(r.max_eigenvalue_statistic)
                        )}
                      </td>
                      <td className="px-2 py-1.5 text-right text-gray-500">{fmt(r.max_eigen_crit_10pct)}</td>
                      <td className="px-2 py-1.5 text-right text-gray-500">{fmt(r.max_eigen_crit_5pct)}</td>
                      <td className="px-2 py-1.5 text-right text-gray-500">{fmt(r.max_eigen_crit_1pct)}</td>
                    </>
                  )}
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>

      <p className="text-[11px] text-gray-600 leading-relaxed">{note}</p>
      <p className="text-[11px] text-gray-600 mt-2">
        * next to trace (or max eigen) marks the row selected at 5% by Johansen’s sequential rule (Stata vecrank).
      </p>
    </div>
  );
};
