import React, { useMemo, useState } from 'react';
import katex from 'katex';
import 'katex/dist/katex.min.css';
import { OverlayScrollbar } from '@/shared/ui/OverlayScrollbar';
import type { Coefficient } from './shared/types';

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

interface VariableMapping {
  symbol: string;
  variable: string;
  category?: string;
  coef: number;
}

/** Build η (linear predictor) terms for expanded form */
function buildEtaTerms(coefficients: Coefficient[]): string {
  const terms: string[] = [];
  for (const c of coefficients) {
    const coefStr = formatNum(c.coef);
    if (c.variable === 'const') {
      terms.push(coefStr);
      continue;
    }
    const absCoef = formatNum(Math.abs(c.coef));
    const sign = c.coef >= 0 ? '+' : '-';
    let varLabel: string;
    if (c.category != null) {
      varLabel = `\\mathbb{1}(\\text{${escapeLatex(c.variable)}} = \\text{${escapeLatex(c.category)}})`;
    } else {
      varLabel = `\\text{${escapeLatex(c.variable)}}`;
    }
    if (terms.length === 0) {
      terms.push(`${coefStr} \\cdot ${varLabel}`);
    } else {
      terms.push(`${sign} ${absCoef} \\cdot ${varLabel}`);
    }
  }
  return terms.join(' ');
}

/** Multi-line formula, no chained equality. Uses \\begin{gathered} for center alignment. */
function buildExpandedLatex(
  endogName: string,
  coefficients: Coefficient[],
  modelType: 'Logit' | 'Probit'
): string {
  const etaTerms = buildEtaTerms(coefficients);
  const yPart = `P(\\text{${escapeLatex(endogName)}}=1 \\mid x)`;
  if (modelType === 'Logit') {
    return `\\begin{gathered}
  ${yPart} = \\sigma(\\eta) \\\\
  \\sigma(\\eta) = \\frac{1}{1+e^{-\\eta}} \\\\
  \\eta = ${etaTerms}
\\end{gathered}`;
  }
  return `\\begin{gathered}
  ${yPart} = \\Phi(\\eta) \\\\
  \\Phi(\\eta) = \\int_{-\\infty}^{\\eta} \\frac{1}{\\sqrt{2\\pi}} e^{-t^2/2} \\, \\mathrm{d}t \\\\
  \\eta = ${etaTerms}
\\end{gathered}`;
}

function buildSymbolicData(endogName: string, coefficients: Coefficient[], modelType: 'Logit' | 'Probit') {
  const mappings: VariableMapping[] = [];
  const terms: string[] = [];
  let xi = 1;

  mappings.push({ symbol: 'y', variable: endogName, coef: NaN });

  for (const c of coefficients) {
    if (c.variable === 'const') {
      mappings.push({ symbol: '\\beta_0', variable: 'const', coef: c.coef });
      terms.push('\\beta_0');
      continue;
    }
    const sym = `x_{${xi}}`;
    const beta = `\\beta_{${xi}}`;
    mappings.push({ symbol: sym, variable: c.variable, category: c.category, coef: c.coef });
    terms.push(`${beta} ${sym}`);
    xi++;
  }

  const etaPart = terms.join(' + ');
  const yPart = `P(y=1 \\mid x)`;
  const latex =
    modelType === 'Logit'
      ? `\\begin{gathered}
  ${yPart} = \\sigma(\\eta) \\\\
  \\sigma(\\eta) = \\frac{1}{1+e^{-\\eta}} \\\\
  \\eta = ${etaPart}
\\end{gathered}`
      : `\\begin{gathered}
  ${yPart} = \\Phi(\\eta) \\\\
  \\Phi(\\eta) = \\int_{-\\infty}^{\\eta} \\frac{1}{\\sqrt{2\\pi}} e^{-t^2/2} \\, \\mathrm{d}t \\\\
  \\eta = ${etaPart}
\\end{gathered}`;
  return { latex, mappings, terms };
}

function renderKatex(latex: string, displayMode = true): string | null {
  try {
    return katex.renderToString(latex, { displayMode, throwOnError: false });
  } catch {
    return null;
  }
}

function renderInlineKatex(latex: string): string | null {
  return renderKatex(latex, false);
}

interface BinaryFormulaBlockProps {
  modelType: 'Logit' | 'Probit';
  endogName: string;
  coefficients: Coefficient[];
}

const BinaryFormulaBlock: React.FC<BinaryFormulaBlockProps> = ({ modelType, endogName, coefficients }) => {
  const [mode, setMode] = useState<EquationMode>('symbolic');

  const expandedHtml = useMemo(
    () => renderKatex(buildExpandedLatex(endogName, coefficients, modelType)),
    [endogName, coefficients, modelType]
  );

  const { symbolicHtml, mappings } = useMemo(() => {
    const { latex, mappings } = buildSymbolicData(endogName, coefficients, modelType);
    return { symbolicHtml: renderKatex(latex), mappings };
  }, [endogName, coefficients, modelType]);

  const hasCat = useMemo(() => mappings.some((m) => m.category != null), [mappings]);

  return (
    <div className="rounded-xl border border-border bg-card overflow-hidden shadow-sm">
      {/* Toggle */}
      <div className="flex items-center justify-end px-4 pt-3 pb-1">
        <div className="inline-flex rounded-md bg-muted border border-border text-[11px]">
          <button
            onClick={() => setMode('symbolic')}
            className={`px-3 py-1 rounded-l-md transition-colors ${
              mode === 'symbolic'
                ? 'bg-[var(--accent-color)]/20 text-[var(--accent-color)] border-r border-border'
                : 'text-muted-foreground hover:text-foreground border-r border-border'
            }`}
          >
            Symbolic
          </button>
          <button
            onClick={() => setMode('expanded')}
            className={`px-3 py-1 rounded-r-md transition-colors ${
              mode === 'expanded'
                ? 'bg-[var(--accent-color)]/20 text-[var(--accent-color)]'
                : 'text-muted-foreground hover:text-foreground'
            }`}
          >
            Expanded
          </button>
        </div>
      </div>

      {/* Formula */}
      <OverlayScrollbar direction="both">
        <div
          className="px-6 py-5 min-w-full [&_.katex]:text-foreground [&_.katex]:text-[1.05em] [&_.katex-display]:py-3 [&_.katex-display]:leading-relaxed"
          dangerouslySetInnerHTML={{ __html: (mode === 'expanded' ? expandedHtml : symbolicHtml) || '' }}
        />
      </OverlayScrollbar>

      {/* Mapping table (symbolic mode only) */}
      {mode === 'symbolic' && (
        <div className="border-t border-border px-4 pb-4 pt-3">
          <div className="text-[11px] text-muted-foreground uppercase tracking-wider mb-2 px-1">Variable Mapping</div>
          <table className="w-full text-xs">
            <thead>
              <tr className="text-muted-foreground">
                <th className="text-left px-3 py-1.5 font-medium w-20">Symbol</th>
                <th className="text-left px-3 py-1.5 font-medium">Variable</th>
                {hasCat && <th className="text-left px-3 py-1.5 font-medium">Category</th>}
                <th className="text-right px-3 py-1.5 font-medium w-28">Coefficient</th>
              </tr>
            </thead>
            <tbody>
              {mappings.map((m, idx) => {
                const symHtml = renderInlineKatex(m.symbol);
                return (
                  <tr key={idx} className={`border-t border-border ${idx % 2 === 0 ? 'bg-muted/50' : ''}`}>
                    <td className="px-3 py-1.5">
                      {symHtml ? (
                        <span className="[&_.katex]:text-[var(--accent-color)]" dangerouslySetInnerHTML={{ __html: symHtml }} />
                      ) : (
                        <span className="font-mono text-[var(--accent-color)]">{m.symbol}</span>
                      )}
                    </td>
                    <td className="px-3 py-1.5 font-mono text-foreground">{m.variable}</td>
                    {hasCat && (
                      <td className="px-3 py-1.5">
                        {m.category != null ? (
                          <span className="inline-flex px-2 py-0.5 rounded text-[11px] font-mono bg-indigo-500/15 text-indigo-300 border border-indigo-500/25">
                            {m.category}
                          </span>
                        ) : (
                          <span className="text-muted-foreground">—</span>
                        )}
                      </td>
                    )}
                    <td className="text-right px-3 py-1.5 font-mono text-muted-foreground">
                      {isNaN(m.coef) ? '—' : formatNum(m.coef)}
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
};

export default BinaryFormulaBlock;
