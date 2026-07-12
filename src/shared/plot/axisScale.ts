/**
 * 按列选择 D3 坐标轴 scale（numeric / category）。
 *
 * PlotView 中单图组件（Line、BarChart 等）通常整图共用同质 scale，继续在组件内内联即可。
 * 平行坐标等多轴、按列异质 scale 的场景使用本模块，避免 `as unknown as scaleLinear` 逃逸。
 */

import { extent, scaleLinear, scalePoint, type ScaleLinear, type ScalePoint } from 'd3';

export type ColumnAxisKind = 'numeric' | 'category';

export interface NumericColumnAxisScale {
  readonly kind: 'numeric';
  readonly scale: ScaleLinear<number, number>;
}

export interface CategoryColumnAxisScale {
  readonly kind: 'category';
  readonly scale: ScalePoint<string>;
}

export type ColumnAxisScale = NumericColumnAxisScale | CategoryColumnAxisScale;

export function columnAxisKindFromType(type: 'number' | 'string'): ColumnAxisKind {
  return type === 'number' ? 'numeric' : 'category';
}

export interface CreateColumnAxisScaleOptions {
  /** 数值轴 domain 两侧 padding 比例，默认 0.05 */
  paddingRatio?: number;
  /** 分类轴 scalePoint padding，默认 0.1 */
  categoryPadding?: number;
}

/**
 * 为单列构建垂直方向 scale（range 通常为 `[height, 0]`）。
 */
export function createColumnAxisScale(
  kind: ColumnAxisKind,
  values: readonly (string | number | null | undefined)[],
  range: [number, number],
  options?: CreateColumnAxisScaleOptions,
): ColumnAxisScale {
  if (kind === 'numeric') {
    const nums = values.filter(
      (v): v is number => typeof v === 'number' && Number.isFinite(v),
    );
    let lo = 0;
    let hi = 1;
    if (nums.length > 0) {
      const [extLo, extHi] = extent(nums);
      lo = extLo ?? 0;
      hi = extHi ?? lo;
      if (lo === hi) {
        lo -= 1;
        hi += 1;
      }
    }
    const padRatio = options?.paddingRatio ?? 0.05;
    const pad = (hi - lo) * padRatio || 1;
    return {
      kind: 'numeric',
      scale: scaleLinear().domain([lo - pad, hi + pad]).range(range),
    };
  }

  const unique = [...new Set(values.filter((v) => v != null).map(String))];
  return {
    kind: 'category',
    scale: scalePoint<string>()
      .domain(unique)
      .range(range)
      .padding(options?.categoryPadding ?? 0.1),
  };
}

/** 将单元格原始值映射到像素 y；无效值返回 undefined。 */
export function mapColumnAxisValue(
  axis: ColumnAxisScale,
  raw: string | number | null | undefined,
): number | undefined {
  if (raw == null) return undefined;
  if (axis.kind === 'numeric') {
    const n = typeof raw === 'number' ? raw : Number(raw);
    if (!Number.isFinite(n)) return undefined;
    const y = axis.scale(n);
    return y == null || Number.isNaN(y) ? undefined : y;
  }
  const y = axis.scale(String(raw));
  return y == null || Number.isNaN(y) ? undefined : y;
}

/** 仅数值轴生成刻度；分类轴返回空数组（平行坐标不绘分类 tick）。 */
export function numericColumnAxisTicks(axis: ColumnAxisScale, count = 4): number[] {
  return axis.kind === 'numeric' ? axis.scale.ticks(count) : [];
}
