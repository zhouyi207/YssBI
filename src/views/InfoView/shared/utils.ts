/** 共享工具函数 */

export {
  coerceFiniteNumber,
  formatNum,
  formatNullableNum,
  formatPercent,
} from './formatStat';

/** 将线性形式字符串转为 LaTeX（变量名 → β 系数） */
export function linearFormToLatex(form: string, paramNames: string[]): string {
  const sorted = [...paramNames].sort((a, b) => b.length - a.length);
  let latex = form;
  for (const p of sorted) {
    const escaped = p.replace(/_/g, '\\_');
    const replacement = `\\beta_{\\text{${escaped}}}`;
    const regex = new RegExp(`\\b${p.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}\\b`, 'g');
    latex = latex.replace(regex, replacement);
  }
  latex = latex.replace(/\*/g, ' \\cdot ');
  latex = latex.replace(/ ≠ /g, ' \\neq ');
  latex = latex.replace(/ ≤ /g, ' \\leq ');
  latex = latex.replace(/ ≥ /g, ' \\geq ');
  return latex;
}

import katex from 'katex';

export function renderHypothesisLatex(latex: string): string | null {
  try {
    return katex.renderToString(latex, { displayMode: true, throwOnError: false });
  } catch {
    return null;
  }
}
