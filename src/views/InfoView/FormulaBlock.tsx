import React, { useMemo, useState } from 'react';
import katex from 'katex';
import 'katex/dist/katex.min.css';
import { OverlayScrollbar } from '@/shared/ui/OverlayScrollbar';
import { InfoSegmentedToggle } from './shared/InfoViewControls';
import { FormulaMappingTable } from './shared/FormulaMappingTable';
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

function buildExpandedLatex(endogName: string, coefficients: Coefficient[]): string {
  const lhs = `\\text{${escapeLatex(endogName)}}`;
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

  return `${lhs} = ${terms.join(' ')} + \\varepsilon`;
}

function buildExpandedLatexWithAR1(endogName: string, coefficients: Coefficient[], rho: number): string {
  const mainEq = buildExpandedLatex(endogName, coefficients).replace('+ \\varepsilon', '+ u_t');
  const rhoStr = formatNum(rho);
  return `${mainEq},\\quad u_t = ${rhoStr} \\cdot u_{t-1} + e_t`;
}

function buildSymbolicData(endogName: string, coefficients: Coefficient[], ar1Rho?: number) {
  const hasAR1 = ar1Rho != null;
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

  if (hasAR1) {
    mappings.push({ symbol: '\\rho', variable: 'rho', coef: ar1Rho ?? NaN });
  }
  const latex = hasAR1
    ? buildSymbolicLatexWithAR1(terms)
    : `y = ${terms.join(' + ')} + \\varepsilon`;
  return { latex, mappings, terms };
}

function buildSymbolicLatexWithAR1(terms: string[]): string {
  return `y = ${terms.join(' + ')} + u_t,\\quad u_t = \\rho \\, u_{t-1} + e_t`;
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

interface FormulaBlockProps {
  endogName: string;
  coefficients: Coefficient[];
  /** AR(1) 自相关参数 ρ，Prais-Winsten/Cochrane-Orcutt 时传入 */
  ar1Rho?: number;
}

const FormulaBlock: React.FC<FormulaBlockProps> = ({ endogName, coefficients, ar1Rho }) => {
  const [mode, setMode] = useState<EquationMode>('symbolic');

  const expandedHtml = useMemo(
    () =>
      ar1Rho != null
        ? renderKatex(buildExpandedLatexWithAR1(endogName, coefficients, ar1Rho))
        : renderKatex(buildExpandedLatex(endogName, coefficients)),
    [endogName, coefficients, ar1Rho]
  );

  const { symbolicHtml, mappings } = useMemo(() => {
    const { latex, mappings } = buildSymbolicData(endogName, coefficients, ar1Rho);
    return { symbolicHtml: renderKatex(latex), mappings };
  }, [endogName, coefficients, ar1Rho]);

  const hasCat = useMemo(() => mappings.some((m) => m.category != null), [mappings]);

  return (
    <div className="rounded-lg border border-border bg-card overflow-hidden">
      {/* Toggle */}
      <div className="flex items-center justify-end px-4 pt-3 pb-0">
        <InfoSegmentedToggle
          value={mode}
          onValueChange={setMode}
          options={[
            { value: 'symbolic', label: 'Symbolic' },
            { value: 'expanded', label: 'Expanded' },
          ]}
        />
      </div>

      {/* Formula */}
      <OverlayScrollbar direction="horizontal">
        <div
          className="px-6 py-4 w-max min-w-full [&_.katex]:text-foreground"
          dangerouslySetInnerHTML={{ __html: (mode === 'expanded' ? expandedHtml : symbolicHtml) || '' }}
        />
      </OverlayScrollbar>

      {/* Mapping table (symbolic mode only) */}
      {mode === 'symbolic' && (
        <div className="border-t border-border px-4 pb-4 pt-3">
          <div className="mb-2 px-1 text-[11px] uppercase tracking-wider text-muted-foreground">Variable Mapping</div>
          <FormulaMappingTable
            mappings={mappings}
            hasCat={hasCat}
            formatNum={formatNum}
            renderSymbol={(symbol) => {
              const symHtml = renderInlineKatex(symbol);
              return symHtml ? (
                <span className="[&_.katex]:text-[var(--accent-color)]" dangerouslySetInnerHTML={{ __html: symHtml }} />
              ) : (
                <span className="font-mono text-[var(--accent-color)]">{symbol}</span>
              );
            }}
          />
        </div>
      )}
    </div>
  );
};

export default FormulaBlock;
