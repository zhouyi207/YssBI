/** 共享工具函数 */

import type { Coefficient } from '@/shared/types/report';

export {
  coerceFiniteNumber,
  formatNum,
  formatNullableNum,
  formatPercent,
} from './formatStat';

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

/** 高斯核 K(u) = (1/sqrt(2π)) * exp(-u²/2) */
const INV_SQRT_2PI = 0.3989422804014327;
function gaussianKernel(u: number): number {
  return INV_SQRT_2PI * Math.exp(-0.5 * u * u);
}

/** Silverman 带宽: h = 1.06 * σ * n^(-1/5) */
function silvermanBandwidth(values: number[]): number {
  const n = values.length;
  if (n < 2) return 1;
  const mean = values.reduce((a, b) => a + b, 0) / n;
  const variance = values.reduce((s, v) => s + (v - mean) ** 2, 0) / (n - 1);
  const sigma = Math.sqrt(variance);
  if (sigma <= 0 || !Number.isFinite(sigma)) return 1;
  return 1.06 * sigma * Math.pow(n, -0.2);
}

/** 在 x 处计算 KDE: f(x) = (1/(n*h)) * Σ K((x - xi)/h) */
function kdeAt(x: number, values: number[], h: number): number {
  if (values.length === 0 || h <= 0) return 0;
  const n = values.length;
  const sum = values.reduce((s, xi) => s + gaussianKernel((x - xi) / h), 0);
  return sum / (n * h);
}

/** 从原始数据计算 KDE 曲线，返回 [{x, y}] 供 KDE 组件使用
 * @param minX 可选，x 轴下界（如 leverage 非负则传 0）
 */
export function computeKDE(
  values: number[],
  gridPoints = 256,
  minX?: number
): { x: number; y: number }[] {
  const valid = values.filter((v) => Number.isFinite(v));
  if (valid.length < 2) return [];
  const h = silvermanBandwidth(valid);
  const minVal = Math.min(...valid);
  const maxVal = Math.max(...valid);
  const range = maxVal - minVal;
  const pad = Math.max(range * 0.15, h * 2, 0.1);
  const xMin = minX != null ? Math.max(minX, minVal - pad) : minVal - pad;
  const xMax = maxVal + pad;
  const data: { x: number; y: number }[] = [];
  for (let i = 0; i <= gridPoints; i++) {
    const t = i / gridPoints;
    const x = xMin + t * (xMax - xMin);
    const y = kdeAt(x, valid, h);
    data.push({ x, y: Math.max(0, y) });
  }
  return data;
}
