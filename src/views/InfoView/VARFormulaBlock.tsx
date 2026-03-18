import React, { useMemo, useState } from 'react';
import katex from 'katex';
import 'katex/dist/katex.min.css';
import { OverlayScrollbar } from '@/shared/ui/OverlayScrollbar';
import type { VARCoefDisplay } from './shared/types';

function formatNum(value: number, decimals = 4): string {
  if (Math.abs(value) < 0.0001 && value !== 0) {
    return value.toExponential(3);
  }
  return value.toFixed(decimals);
}

function escapeLatex(s: string): string {
  return s.replace(/[_{}\\^~&%$#]/g, (ch) => `\\${ch}`);
}

type EquationMode = 'expanded' | 'symbolic';

function renderKatex(latex: string, displayMode = true): string | null {
  try {
    return katex.renderToString(latex, { displayMode, throwOnError: false });
  } catch {
    return null;
  }
}

/** 从系数变量名解析滞后阶数 */
function parseLagsFromCoeffs(coefficients: VARCoefDisplay[]): number[] {
  const lags = new Set<number>();
  for (const c of coefficients) {
    const m = c.variable.match(/^L(\d+)\./);
    if (m) lags.add(parseInt(m[1], 10));
  }
  return Array.from(lags).sort((a, b) => a - b);
}

/** 按方程分组系数 */
function groupByEquation(coefficients: VARCoefDisplay[]): Map<string, VARCoefDisplay[]> {
  const map = new Map<string, VARCoefDisplay[]>();
  for (const c of coefficients) {
    const list = map.get(c.eq_name) ?? [];
    list.push(c);
    map.set(c.eq_name, list);
  }
  return map;
}

/** 将 L1.var 转为 LaTeX 下标形式 var_{t-1} */
function varToLatex(varName: string): string {
  const m = varName.match(/^L(\d+)\.(.+)$/);
  if (m) {
    const lag = m[1];
    const name = escapeLatex(m[2]);
    return `\\text{${name}}_{t-${lag}}`;
  }
  if (varName === 'const') return '';
  return `\\text{${escapeLatex(varName)}}`;
}

function buildSymbolicLatex(lags: number[]): string {
  const sumTerms =
    lags.length > 0
      ? lags.map((l) => `A_{${l}} \\mathbf{y}_{t-${l}}`).join(' + ')
      : 'A_1 \\mathbf{y}_{t-1} + \\cdots + A_p \\mathbf{y}_{t-p}';
  return `\\begin{gathered}
\\mathbf{y}_t = \\mathbf{v} + ${sumTerms} + \\mathbf{u}_t \\\\
\\mathbf{y}_t \\in \\mathbb{R}^K,\\quad A_l \\in \\mathbb{R}^{K \\times K},\\quad \\mathbf{u}_t \\sim \\text{WN}(0,\\Sigma)
\\end{gathered}`;
}

function buildExpandedLatex(
  varNames: string[],
  eqCoeffs: Map<string, VARCoefDisplay[]>
): string {
  const lines: string[] = [];
  for (const eqName of varNames) {
    const coeffs = [...(eqCoeffs.get(eqName) ?? [])].sort((a, b) =>
      (a.variable === 'const' ? 0 : 1) - (b.variable === 'const' ? 0 : 1)
    );
    const terms: string[] = [];
    for (const c of coeffs) {
      if (c.variable === 'const') {
        terms.push(formatNum(c.coef));
        continue;
      }
      const absCoef = formatNum(Math.abs(c.coef));
      const sign = c.coef >= 0 ? '+' : '-';
      const varLabel = varToLatex(c.variable);
      if (varLabel) {
        if (terms.length === 0) {
          terms.push(`${formatNum(c.coef)} \\cdot ${varLabel}`);
        } else {
          terms.push(`${sign} ${absCoef} \\cdot ${varLabel}`);
        }
      }
    }
    const lhs = `\\text{${escapeLatex(eqName)}}_t`;
    const rhs = `${terms.join(' \\; ')} + u_t`;
    lines.push(`${lhs} &= ${rhs}`);
  }
  return `\\begin{aligned}\n${lines.join(' \\\\\n')}\n\\end{aligned}`;
}

interface VARFormulaBlockProps {
  varNames: string[];
  coefficients: VARCoefDisplay[];
}

const VARFormulaBlock: React.FC<VARFormulaBlockProps> = ({ varNames, coefficients }) => {
  const [mode, setMode] = useState<EquationMode>('symbolic');

  const { symbolicHtml, expandedHtml } = useMemo(() => {
    const lags = parseLagsFromCoeffs(coefficients);
    const eqCoeffs = groupByEquation(coefficients);
    const symbolicHtml = renderKatex(buildSymbolicLatex(lags));
    const expandedHtml = renderKatex(buildExpandedLatex(varNames, eqCoeffs));
    return { symbolicHtml, expandedHtml };
  }, [varNames, coefficients]);

  return (
    <div className="rounded-lg border border-gray-800/50 bg-[#13151a] overflow-hidden">
      {/* Toggle */}
      <div className="flex items-center justify-end px-4 pt-3 pb-1">
        <div className="inline-flex rounded-md bg-[#1a1d23] border border-gray-800/50 text-[11px]">
          <button
            onClick={() => setMode('symbolic')}
            className={`px-3 py-1 rounded-l-md transition-colors ${
              mode === 'symbolic'
                ? 'bg-[var(--accent-color)]/20 text-[var(--accent-color)] border-r border-gray-800/50'
                : 'text-gray-500 hover:text-gray-300 border-r border-gray-800/50'
            }`}
          >
            Symbolic
          </button>
          <button
            onClick={() => setMode('expanded')}
            className={`px-3 py-1 rounded-r-md transition-colors ${
              mode === 'expanded'
                ? 'bg-[var(--accent-color)]/20 text-[var(--accent-color)]'
                : 'text-gray-500 hover:text-gray-300'
            }`}
          >
            Expanded
          </button>
        </div>
      </div>

      {/* Formula */}
      <OverlayScrollbar direction="both">
        <div
          className="px-6 py-5 min-w-full [&_.katex]:text-gray-200 [&_.katex]:text-[1.05em] [&_.katex-display]:py-3 [&_.katex-display]:leading-relaxed"
          dangerouslySetInnerHTML={{ __html: (mode === 'expanded' ? expandedHtml : symbolicHtml) || '' }}
        />
      </OverlayScrollbar>

      {/* Symbolic mode: variable mapping */}
      {mode === 'symbolic' && (
        <div className="border-t border-gray-800/40 px-4 pb-4 pt-3">
          <div className="text-[11px] text-gray-500 uppercase tracking-wider mb-2 px-1">Variable Mapping</div>
          <table className="w-full text-xs">
            <thead>
              <tr className="text-gray-500">
                <th className="text-left px-3 py-1.5 font-medium w-24">Symbol</th>
                <th className="text-left px-3 py-1.5 font-medium">Meaning</th>
              </tr>
            </thead>
            <tbody>
              <tr className="border-t border-gray-800/20 bg-[#15171d]/50">
                <td className="px-3 py-1.5 font-mono text-[var(--accent-color)]">y_t</td>
                <td className="px-3 py-1.5 text-gray-400">K×1 vector of endogenous variables</td>
              </tr>
              <tr className="border-t border-gray-800/20">
                <td className="px-3 py-1.5 font-mono text-[var(--accent-color)]">A_l</td>
                <td className="px-3 py-1.5 text-gray-400">K×K coefficient matrix at lag l</td>
              </tr>
              <tr className="border-t border-gray-800/20 bg-[#15171d]/50">
                <td className="px-3 py-1.5 font-mono text-[var(--accent-color)]">v</td>
                <td className="px-3 py-1.5 text-gray-400">Constant vector</td>
              </tr>
              <tr className="border-t border-gray-800/20">
                <td className="px-3 py-1.5 font-mono text-[var(--accent-color)]">u_t</td>
                <td className="px-3 py-1.5 text-gray-400">Innovation vector, white noise</td>
              </tr>
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
};

export default VARFormulaBlock;
