import React from 'react';
import { formatNum } from './utils';
import type { BreuschPaganTests } from './types';

export { formatNum };

export function SignificanceStars({ pValue }: { pValue: number }) {
  if (pValue < 0.001) return <span className="text-yellow-400 font-bold ml-1">***</span>;
  if (pValue < 0.01) return <span className="text-yellow-400 font-bold ml-1">**</span>;
  if (pValue < 0.05) return <span className="text-yellow-400 font-bold ml-1">*</span>;
  if (pValue < 0.1) return <span className="text-gray-500 ml-1">.</span>;
  return null;
}

export function RSquaredBadge({ value }: { value: number }) {
  let color = 'bg-red-500/20 text-red-400 border-red-500/30';
  if (value >= 0.7) color = 'bg-emerald-500/20 text-emerald-400 border-emerald-500/30';
  else if (value >= 0.4) color = 'bg-yellow-500/20 text-yellow-400 border-yellow-500/30';

  return (
    <span className={`inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-semibold border ${color}`}>
      R² = {value.toFixed(3)}
    </span>
  );
}

export function StatCard({ label, value, sub }: { label: string; value: string | number; sub?: string }) {
  return (
    <div className="bg-[#1a1d23] rounded-lg px-4 py-3 border border-gray-800/50">
      <div className="text-[11px] text-gray-500 uppercase tracking-wider mb-1">{label}</div>
      <div className="text-white font-mono text-sm font-medium">{value}</div>
      {sub && <div className="text-[10px] text-gray-600 mt-0.5">{sub}</div>}
    </div>
  );
}

export function SectionHeader({ title, icon }: { title: string; icon: React.ReactNode }) {
  return (
    <div className="flex items-center gap-2 mb-3 mt-6 first:mt-0">
      <div className="text-[var(--accent-color)]">{icon}</div>
      <h3 className="text-sm font-semibold text-gray-300 uppercase tracking-wider">{title}</h3>
      <div className="flex-1 h-px bg-gray-800 ml-2"></div>
    </div>
  );
}

export function InfoRow({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="bg-[#13151a] px-4 py-2.5 flex justify-between">
      <span className="text-gray-500 text-xs">{label}</span>
      <span className="text-white text-xs font-mono font-medium">{children}</span>
    </div>
  );
}

const BP_VARIANTS: { key: keyof BreuschPaganTests; label: string }[] = [
  { key: 'stata', label: 'estat hettest' },
  { key: 'koenker', label: 'estat hettest, iid' },
  { key: 'stata_rhs', label: 'estat hettest, rhs' },
  { key: 'koenker_rhs', label: 'estat hettest, rhs iid' },
];

export { BP_VARIANTS };

export interface Chi2TestCard {
  label: string;
  chi2: number;
  df: number;
  p_value: number;
}

export function Chi2TestCards({ cards }: { cards: Chi2TestCard[] }) {
  return (
    <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
      {cards.map((c) => {
        const reject = c.p_value < 0.05;
        return (
          <div
            key={c.label}
            className="rounded-lg border border-gray-800/50 bg-[#1a1d23] px-4 py-3 hover:border-gray-700/50 transition-colors"
          >
            <div className="text-[11px] text-gray-500 font-mono mb-2">{c.label}</div>
            <div className="flex flex-wrap items-baseline gap-x-4 gap-y-1 text-xs">
              <span className="text-gray-400">
                chi2 = <span className="font-mono text-white">{formatNum(c.chi2)}</span>
              </span>
              <span className="text-gray-400">
                df = <span className="font-mono text-gray-300">{c.df}</span>
              </span>
              <span className="text-gray-400">
                p = <span className={`font-mono ${reject ? 'text-emerald-400' : 'text-gray-400'}`}>{formatNum(c.p_value)}</span>
              </span>
            </div>
            <div className="mt-1.5 text-[10px]">
              {reject ? (
                <span className="text-amber-400">拒绝 H0</span>
              ) : (
                <span className="text-gray-500">不拒绝 H0</span>
              )}
            </div>
          </div>
        );
      })}
    </div>
  );
}
