/** 共享工具函数 */

import type { Coefficient } from './types';

export function formatNum(value: number, decimals = 4): string {
  if (Math.abs(value) < 0.0001 && value !== 0) {
    return value.toExponential(3);
  }
  return value.toFixed(decimals);
}

/** 从系数表构建 param_names（与 exog 列序一致） */
export function buildParamNames(coefficients: Coefficient[]): string[] {
  return coefficients.map((c) =>
    c.category != null ? `${c.variable}_${c.category}` : c.variable
  );
}

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
