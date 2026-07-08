import React from 'react';
import { Badge } from '@/components/ui/badge';
import { Card, CardContent } from '@/components/ui/card';
import { formatNum, formatNullableNum, coerceFiniteNumber } from './utils';
import type { BreuschPaganTests } from './types';

export { formatNum, formatNullableNum, formatPercent, coerceFiniteNumber } from './utils';

export function SignificanceStars({ pValue }: { pValue: number }) {
  if (pValue < 0.001) return <span className="text-yellow-400 font-bold ml-1">***</span>;
  if (pValue < 0.01) return <span className="text-yellow-400 font-bold ml-1">**</span>;
  if (pValue < 0.05) return <span className="text-yellow-400 font-bold ml-1">*</span>;
  if (pValue < 0.1) return <span className="text-muted-foreground ml-1">.</span>;
  return null;
}

export function RSquaredBadge({ value }: { value: unknown }) {
  const n = coerceFiniteNumber(value);
  const variant =
    n == null ? 'destructive' : n >= 0.7 ? 'success' : n >= 0.4 ? 'warning' : 'destructive';

  return (
    <Badge variant={variant} className="rounded-full px-2.5 py-0.5 text-xs font-semibold normal-case tracking-normal">
      R² = {formatNullableNum(value, 3, 'N/A')}
    </Badge>
  );
}

export function StatValue({
  value,
  decimals = 4,
  fallback = '—',
}: {
  value: unknown;
  decimals?: number;
  fallback?: string;
}) {
  return <>{formatNullableNum(value, decimals, fallback)}</>;
}

export function StatCard({ label, value, sub }: { label: string; value: string | number; sub?: string }) {
  return (
    <Card className="rounded-lg py-0 shadow-none">
      <CardContent className="px-4 py-3">
        <div className="mb-1 text-[11px] uppercase tracking-wider text-muted-foreground">{label}</div>
        <div className="font-mono text-sm font-medium text-foreground">{value}</div>
        {sub && <div className="mt-0.5 text-[10px] text-muted-foreground">{sub}</div>}
      </CardContent>
    </Card>
  );
}

export function SectionHeader({ title, icon }: { title: string; icon: React.ReactNode }) {
  return (
    <div className="flex items-center gap-2 mb-3 mt-6 first:mt-0">
      <div className="text-[var(--accent-color)]">{icon}</div>
      <h3 className="text-sm font-semibold text-foreground uppercase tracking-wider">{title}</h3>
      <div className="flex-1 h-px bg-border ml-2"></div>
    </div>
  );
}

export function InfoRow({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="bg-card px-4 py-2.5 flex justify-between">
      <span className="text-muted-foreground text-xs">{label}</span>
      <span className="text-foreground text-xs font-mono font-medium">{children}</span>
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
          <Card key={c.label} className="rounded-lg bg-muted py-0 shadow-none transition-colors hover:border-border">
            <CardContent className="px-4 py-3">
              <div className="mb-2 font-mono text-[11px] text-muted-foreground">{c.label}</div>
              <div className="flex flex-wrap items-baseline gap-x-4 gap-y-1 text-xs">
                <span className="text-muted-foreground">
                  chi2 = <span className="font-mono text-foreground">{formatNum(c.chi2)}</span>
                </span>
                <span className="text-muted-foreground">
                  df = <span className="font-mono text-foreground">{c.df}</span>
                </span>
                <span className="text-muted-foreground">
                  p = <span className={`font-mono ${reject ? 'text-emerald-400' : 'text-muted-foreground'}`}>{formatNum(c.p_value)}</span>
                </span>
              </div>
              <div className="mt-1.5 text-[10px]">
                {reject ? (
                  <span className="text-amber-400">拒绝 H0</span>
                ) : (
                  <span className="text-muted-foreground">不拒绝 H0</span>
                )}
              </div>
            </CardContent>
          </Card>
        );
      })}
    </div>
  );
}

export interface FTestCard {
  label: string;
  f_stat: number;
  df1: number;
  df2: number;
  p_value: number;
}

const OV_VARIANTS: { key: 'default' | 'rhs'; label: string }[] = [
  { key: 'default', label: 'estat ovtest' },
  { key: 'rhs', label: 'estat ovtest, rhs' },
];

export { OV_VARIANTS };

export function FTestCards({ cards }: { cards: FTestCard[] }) {
  return (
    <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
      {cards.map((c) => {
        const reject = c.p_value < 0.05;
        return (
          <Card key={c.label} className="rounded-lg bg-muted py-0 shadow-none transition-colors hover:border-border">
            <CardContent className="px-4 py-3">
              <div className="mb-2 font-mono text-[11px] text-muted-foreground">{c.label}</div>
              <div className="flex flex-wrap items-baseline gap-x-4 gap-y-1 text-xs">
                <span className="text-muted-foreground">
                  F({c.df1},{c.df2}) = <span className="font-mono text-foreground">{formatNum(c.f_stat)}</span>
                </span>
                <span className="text-muted-foreground">
                  p = <span className={`font-mono ${reject ? 'text-emerald-400' : 'text-muted-foreground'}`}>{formatNum(c.p_value)}</span>
                </span>
              </div>
              <div className="mt-1.5 text-[10px]">
                {reject ? (
                  <span className="text-amber-400">拒绝 H0（模型可能有遗漏变量或函数形式误设）</span>
                ) : (
                  <span className="text-muted-foreground">不拒绝 H0</span>
                )}
              </div>
            </CardContent>
          </Card>
        );
      })}
    </div>
  );
}
