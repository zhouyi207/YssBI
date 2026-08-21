import { useMemo } from 'react';
import katex from 'katex';
import 'katex/dist/katex.min.css';
import type { Translation } from './types';

const LATEX_GREEK_SYMBOLS = new Set([
  'alpha', 'beta', 'gamma', 'delta', 'epsilon', 'zeta', 'eta', 'theta', 'iota', 'kappa',
  'lambda', 'mu', 'nu', 'xi', 'pi', 'rho', 'sigma', 'tau', 'upsilon', 'phi',
  'chi', 'psi', 'omega',
]);

export function latexSymbol(value: string): string {
  if (LATEX_GREEK_SYMBOLS.has(value)) return `\\${value}`;
  const indexed = value.match(/^([A-Za-z]+)_(?:\{([A-Za-z0-9_]+)\}|([A-Za-z0-9_]+))$/);
  if (!indexed) return value;
  const [, base, bracedIndex, plainIndex] = indexed;
  const renderedBase = LATEX_GREEK_SYMBOLS.has(base) ? `\\${base}` : base;
  return `${renderedBase}_{${bracedIndex ?? plainIndex}}`;
}

export function RecognizedSymbols({ symbols, t }: { symbols: string[]; t: Translation }) {
  const uniqueSymbols = Array.from(new Set(symbols)).sort();
  return (
    <div className="flex flex-wrap items-center gap-1.5 text-xs text-muted-foreground">
      <span>{t('bayes.formula.recognizedSymbols')}</span>
      {uniqueSymbols.length > 0
        ? uniqueSymbols.map(symbol => (
          <span key={symbol} className="rounded border border-border bg-muted/30 px-1.5 py-0.5 text-foreground">
            <LatexInline formulaText={latexSymbol(symbol)} />
          </span>
        ))
        : <span>{t('bayes.common.none')}</span>}
    </div>
  );
}

export function LatexFormulaPreview({ formulaText }: { formulaText: string }) {
  const html = useMemo(() => renderLatex(formulaText, true), [formulaText]);
  return (
    <div
      className="rounded-md border border-border bg-muted/30 px-3 py-3 text-sm overflow-x-auto [&_.katex]:text-foreground [&_.katex-display]:my-0"
      dangerouslySetInnerHTML={{ __html: html }}
    />
  );
}

export function LatexInline({ formulaText }: { formulaText: string }) {
  const html = useMemo(() => renderLatex(formulaText, false), [formulaText]);
  return <span className="[&_.katex]:text-foreground" dangerouslySetInnerHTML={{ __html: html }} />;
}

function renderLatex(formulaText: string, displayMode: boolean): string {
  const latex = formulaText.trim() || '\\cdots';
  try {
    return katex.renderToString(latex, {
      displayMode,
      throwOnError: false,
    });
  } catch {
    return escapeHtml(latex);
  }
}

function escapeHtml(value: string): string {
  return value.replace(/[&<>"]/g, (character) => {
    switch (character) {
      case '&':
        return '&amp;';
      case '<':
        return '&lt;';
      case '>':
        return '&gt;';
      case '"':
        return '&quot;';
      default:
        return character;
    }
  });
}
