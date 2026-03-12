import React, { useEffect, useMemo, useState } from 'react';
import { SectionHeader, formatNum } from './RegressionShared';
import { buildParamNames } from './utils';
import { parseAtValues } from '@/services/stats/parseAtService';
import type { OLSResultData } from './types';

/** Standard normal PDF φ(x) */
function phi(x: number): number {
  return Math.exp(-0.5 * x * x) / Math.sqrt(2 * Math.PI);
}

/** Standard normal CDF Φ(x) */
function Phi(x: number): number {
  return 0.5 * (1 + erf(x / Math.SQRT2));
}
function erf(x: number): number {
  const a1 = 0.254829592, a2 = -0.284496736, a3 = 1.421413741, a4 = -1.453152027, a5 = 1.061405429;
  const p = 0.3275911;
  const t = 1.0 / (1.0 + p * Math.abs(x));
  const poly = (((a5 * t + a4) * t + a3) * t + a2) * t + a1;
  const y = 1.0 - poly * t * Math.exp(-x * x);
  return x >= 0 ? y : -y;
}

/** Build evaluation point: exog_means with at overrides */
function buildEvalPoint(
  exogMeans: number[],
  paramNames: string[],
  atOverrides: Record<string, number>
): number[] {
  return paramNames.map((p, i) => (atOverrides[p] !== undefined ? atOverrides[p] : exogMeans[i] ?? 0));
}

type MarginType =
  | 'dydx_avg'
  | 'dydx_means'
  | 'dydx_at'
  | 'eyex'
  | 'eydx'
  | 'dyex';

const MARGIN_OPTIONS: { value: MarginType; label: string; needsAt: boolean }[] = [
  { value: 'dydx_avg', label: 'margins, dydx(*)', needsAt: false },
  { value: 'dydx_means', label: 'margins, dydx(*) at means', needsAt: false },
  { value: 'dydx_at', label: 'margins, dydx(*) at (specify)', needsAt: true },
  { value: 'eyex', label: 'margins, eyex(*)', needsAt: false },
  { value: 'eydx', label: 'margins, eydx(*)', needsAt: false },
  { value: 'dyex', label: 'margins, dyex(*)', needsAt: false },
];

function computeMargins(
  marginType: MarginType,
  modelType: 'Logit' | 'Probit',
  betas: number[],
  exog: number[][],
  exogMeans: number[],
  paramNames: string[],
  atOverrides: Record<string, number>
): { variable: string; margin: number }[] {
  const k = betas.length;
  const isLogit = modelType === 'Logit';

  const getScale = (eta: number, p: number): number => {
    if (isLogit) return p * (1 - p);
    return phi(eta);
  };

  const getP = (eta: number): number => {
    if (isLogit) return 1 / (1 + Math.exp(-eta));
    return Phi(eta);
  };

  const dydxAt = (x: number[], j: number): number => {
    const eta = x.reduce((s, xi, i) => s + xi * betas[i], 0);
    const p = getP(eta);
    const scale = getScale(eta, p);
    return scale * betas[j];
  };

  const varIndices = paramNames.map((_, i) => i);
  const evalPoint = buildEvalPoint(exogMeans, paramNames, atOverrides);

  // dydx_avg: average marginal effects over sample
  if (marginType === 'dydx_avg') {
    const n = exog.length;
    return varIndices.map((j) => {
      const avg = exog.reduce((s, row) => s + dydxAt(row, j), 0) / n;
      return { variable: paramNames[j], margin: avg };
    });
  }

  // Evaluate at point (means or at overrides)
  const etaEval = evalPoint.reduce((s, xi, i) => s + xi * betas[i], 0);
  const pEval = getP(etaEval);
  const scaleEval = getScale(etaEval, pEval);

  return varIndices.map((j) => {
    const dydx = scaleEval * betas[j];
    const xj = evalPoint[j];
    let margin: number;
    if (marginType === 'dydx_means' || marginType === 'dydx_at') {
      margin = dydx;
    } else if (marginType === 'eyex') {
      margin = pEval > 1e-10 ? (dydx * xj) / pEval : 0;
    } else if (marginType === 'eydx') {
      margin = pEval > 1e-10 ? dydx / pEval : 0;
    } else {
      margin = dydx * xj;
    }
    return { variable: paramNames[j], margin };
  });
}

export function MarginsBlock({ data }: { data: OLSResultData }) {
  const { model_basic_info: info, coefficients, diagnostic_info: diag, betas } = data;
  const paramNames = useMemo(() => buildParamNames(coefficients), [coefficients]);

  const [marginType, setMarginType] = useState<MarginType>('dydx_avg');
  const [atSpec, setAtSpec] = useState('');
  const [atOverrides, setAtOverrides] = useState<Record<string, number>>({});
  const [atParseError, setAtParseError] = useState<string | null>(null);

  const modelType = info.model_type === 'Probit' ? 'Probit' : 'Logit';
  const exog = diag.exog;
  const exogMeans = diag.exog_means;

  useEffect(() => {
    if (!atSpec.trim()) {
      setAtOverrides({});
      setAtParseError(null);
      return;
    }
    let cancelled = false;
    parseAtValues({ param_names: paramNames, at_spec: atSpec })
      .then((res) => {
        if (!cancelled) {
          setAtOverrides(res.values);
          setAtParseError(null);
        }
      })
      .catch((e) => {
        if (!cancelled) {
          setAtOverrides({});
          setAtParseError(e instanceof Error ? e.message : String(e));
        }
      });
    return () => {
      cancelled = true;
    };
  }, [atSpec, paramNames]);

  const canCompute = useMemo(() => {
    if (!betas || !exog || !exogMeans || exog.length === 0) return false;
    if (marginType === 'dydx_at' && atSpec.trim() && atParseError) return false;
    return true;
  }, [betas, exog, exogMeans, marginType, atSpec, atParseError]);

  const results = useMemo(() => {
    if (!canCompute || !betas || !exog || !exogMeans) return [];
    return computeMargins(
      marginType,
      modelType,
      betas,
      exog,
      exogMeans,
      paramNames,
      atOverrides
    );
  }, [canCompute, marginType, modelType, betas, exog, exogMeans, paramNames, atOverrides]);

  const currentOpt = MARGIN_OPTIONS.find((o) => o.value === marginType);
  const showAtInput = currentOpt?.needsAt ?? false;

  if (!exog || !exogMeans) return null;

  return (
    <div className="mt-6">
      <SectionHeader
        title="Margins (Stata margins)"
        icon={
          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M7 12l3-3 3 3 4-4M8 21l4-4 4 4M3 4h18M4 4h16v12a1 1 0 01-1 1H5a1 1 0 01-1-1V4z" />
          </svg>
        }
      />
      <div className="rounded-lg border border-gray-800/50 bg-[#13151a] p-4 space-y-3">
        <div className="flex flex-wrap gap-3 items-end">
          <div className="flex flex-col gap-1">
            <label className="text-[10px] text-gray-500 uppercase tracking-wider">Command</label>
            <select
              value={marginType}
              onChange={(e) => setMarginType(e.target.value as MarginType)}
              className="px-3 py-2 rounded-md bg-[#1a1d23] border border-gray-700/50 text-sm font-mono text-white focus:outline-none focus:border-[var(--accent-color)]/50 min-w-[220px]"
            >
              {MARGIN_OPTIONS.map((o) => (
                <option key={o.value} value={o.value}>
                  {o.label}
                </option>
              ))}
            </select>
          </div>
          {showAtInput && (
            <div className="flex flex-col gap-1 flex-1 min-w-[200px]">
              <label className="text-[10px] text-gray-500 uppercase tracking-wider">
                at (e.g. x1 = 0, x2 = 1.5)
              </label>
              <input
                type="text"
                value={atSpec}
                onChange={(e) => setAtSpec(e.target.value)}
                placeholder="x1 = 0, x2 = 1.5（与假设检验格式一致）"
                className="px-3 py-2 rounded-md bg-[#1a1d23] border border-gray-700/50 text-sm font-mono text-white placeholder-gray-500 focus:outline-none focus:border-[var(--accent-color)]/50 w-full"
              />
            </div>
          )}
        </div>
        <div className="text-[10px] text-gray-500">
          Param names: {paramNames.join(', ')}
        </div>
        {atParseError && (
          <div className="text-xs text-red-400 font-mono">{atParseError}</div>
        )}
        {results.length > 0 && (
          <div className="rounded-md bg-[#1a1d23] border border-gray-800/50 overflow-hidden mt-2">
            <table className="w-full text-xs">
              <thead>
                <tr className="bg-[#0d0e12]">
                  <th className="text-left px-4 py-2 text-gray-500 font-medium">Variable</th>
                  <th className="text-right px-4 py-2 text-gray-500 font-medium">
                    {marginType.startsWith('dydx') ? 'dY/dX' : marginType === 'eyex' ? 'ey/ex' : marginType === 'eydx' ? 'ey/dx' : 'dy/ex'}
                  </th>
                </tr>
              </thead>
              <tbody>
                {results.map((r) => (
                  <tr key={r.variable} className="border-t border-gray-800/30">
                    <td className="px-4 py-2 font-mono text-gray-300">{r.variable}</td>
                    <td className="px-4 py-2 text-right font-mono text-white">{formatNum(r.margin)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>
    </div>
  );
}
