import React, { useMemo, useState } from 'react';
import katex from 'katex';
import 'katex/dist/katex.min.css';
import { OverlayScrollbar } from '@/shared/ui/OverlayScrollbar';
import { InfoSegmentedToggle } from './shared/InfoViewControls';
import { formatNum } from './shared/utils';
import type { Coefficient } from '@/shared/types/report';
import type { Iv2slsFirstStageResult } from '@/shared/types/report';

function escapeLatex(s: string): string {
  return s.replace(/[_{}\\^~&%$#]/g, (ch) => `\\${ch}`);
}

type EquationMode = 'stage1' | 'stage2' | 'final';

interface FormulaBlock2SLSProps {
  endogName: string;
  /** 第二阶段/最终结构式系数（const, exog, endog） */
  coefficients: Coefficient[];
  /** 第一阶段回归结果 */
  firstStage: Iv2slsFirstStageResult[];
}

function buildStage1Latex(firstStage: Iv2slsFirstStageResult[]): string {
  const lines: string[] = [];
  for (const fs of firstStage) {
    const terms: string[] = [];
    for (let i = 0; i < fs.coefficients.length; i++) {
      const c = fs.coefficients[i];
      const coefStr = formatNum(c.coef);
      const varLabel = `\\text{${escapeLatex(c.variable)}}`;
      if (terms.length === 0) {
        terms.push(`${coefStr} \\cdot ${varLabel}`);
      } else {
        const sign = c.coef >= 0 ? '+' : '-';
        terms.push(`${sign} ${formatNum(Math.abs(c.coef))} \\cdot ${varLabel}`);
      }
    }
    const lhs = `\\widehat{\\text{${escapeLatex(fs.endog_name)}}}`;
    lines.push(`${lhs} = ${terms.join(' ')}`);
  }
  return lines.join(' \\\\ ');
}

function buildStage2Latex(
  endogName: string,
  coefficients: Coefficient[],
  endogVarNames: string[]
): string {
  const lhs = `\\text{${escapeLatex(endogName)}}`;
  const terms: string[] = [];
  for (const c of coefficients) {
    const coefStr = formatNum(c.coef);
    let varLabel: string;
    if (c.variable === 'const') {
      varLabel = '1';
    } else if (endogVarNames.includes(c.variable)) {
      varLabel = `\\widehat{\\text{${escapeLatex(c.variable)}}}`;
    } else {
      varLabel = `\\text{${escapeLatex(c.variable)}}`;
    }
    if (terms.length === 0) {
      terms.push(`${coefStr} \\cdot ${varLabel}`);
    } else {
      const sign = c.coef >= 0 ? '+' : '-';
      terms.push(`${sign} ${formatNum(Math.abs(c.coef))} \\cdot ${varLabel}`);
    }
  }
  return `${lhs} = ${terms.join(' ')} + \\varepsilon`;
}

function buildFinalLatex(endogName: string, coefficients: Coefficient[]): string {
  const lhs = `\\text{${escapeLatex(endogName)}}`;
  const terms: string[] = [];
  for (const c of coefficients) {
    const coefStr = formatNum(c.coef);
    let varLabel: string;
    if (c.variable === 'const') {
      varLabel = '1';
    } else {
      varLabel = `\\text{${escapeLatex(c.variable)}}`;
    }
    if (terms.length === 0) {
      terms.push(`${coefStr} \\cdot ${varLabel}`);
    } else {
      const sign = c.coef >= 0 ? '+' : '-';
      terms.push(`${sign} ${formatNum(Math.abs(c.coef))} \\cdot ${varLabel}`);
    }
  }
  return `${lhs} = ${terms.join(' ')} + \\varepsilon`;
}

function renderKatex(latex: string, displayMode = true): string | null {
  try {
    return katex.renderToString(latex, { displayMode, throwOnError: false });
  } catch {
    return null;
  }
}

const FormulaBlock2SLS: React.FC<FormulaBlock2SLSProps> = ({
  endogName,
  coefficients,
  firstStage,
}) => {
  const [mode, setMode] = useState<EquationMode>('stage1');

  const stage1Html = useMemo(
    () => renderKatex(`\\begin{aligned} ${buildStage1Latex(firstStage)} \\end{aligned}`),
    [firstStage]
  );

  const endogVarNames = useMemo(
    () => firstStage.map((fs) => fs.endog_name),
    [firstStage]
  );

  const stage2Html = useMemo(
    () => renderKatex(buildStage2Latex(endogName, coefficients, endogVarNames)),
    [endogName, coefficients, endogVarNames]
  );

  const finalHtml = useMemo(
    () => renderKatex(buildFinalLatex(endogName, coefficients)),
    [endogName, coefficients]
  );

  const currentHtml =
    mode === 'stage1' ? stage1Html : mode === 'stage2' ? stage2Html : finalHtml;

  const modeLabels: { key: EquationMode; label: string }[] = [
    { key: 'stage1', label: 'Stage 1' },
    { key: 'stage2', label: 'Stage 2' },
    { key: 'final', label: 'Final' },
  ];

  return (
    <div className="rounded-lg border border-border bg-card overflow-hidden">
      {/* Toggle */}
      <div className="flex items-center justify-end px-4 pt-3 pb-0">
        <InfoSegmentedToggle
          value={mode}
          onValueChange={setMode}
          options={modeLabels.map(({ key, label }) => ({ value: key, label }))}
        />
      </div>

      {/* Formula */}
      <OverlayScrollbar direction="horizontal">
        <div
          className="px-6 py-4 w-max min-w-full [&_.katex]:text-foreground"
          dangerouslySetInnerHTML={{ __html: currentHtml || '' }}
        />
      </OverlayScrollbar>

      {/* Mode description */}
      <div className="border-t border-border px-4 pb-4 pt-3">
        <div className="text-[11px] text-muted-foreground uppercase tracking-wider mb-1 px-1">
          {mode === 'stage1' && 'Stage 1: Each endogenous regressed on exog + instruments'}
          {mode === 'stage2' && 'Stage 2: Y regressed on exog + fitted endog (ŷ = Zγ̂)'}
          {mode === 'final' && 'Final (structural): Y = f(exog, endog) with 2SLS coefficients'}
        </div>
      </div>
    </div>
  );
};

export default FormulaBlock2SLS;
