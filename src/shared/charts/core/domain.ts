import type { ChartMargin } from "./types";

export interface ChartBox {
  width: number;
  height: number;
  plotWidth: number;
  plotHeight: number;
}

export function resolveChartBox(
  width: number,
  height: number,
  margin: ChartMargin,
): ChartBox | null {
  const plotWidth = width - margin.left - margin.right;
  const plotHeight = height - margin.top - margin.bottom;

  if (plotWidth <= 0 || plotHeight <= 0) {
    return null;
  }

  return { width, height, plotWidth, plotHeight };
}

export function paddedNumericDomain(
  values: readonly number[],
  ratio: number,
  constantPadding: number,
): [number, number] {
  let minimum: number | undefined;
  let maximum: number | undefined;

  for (const value of values) {
    if (!Number.isFinite(value)) continue;
    minimum = minimum === undefined ? value : Math.min(minimum, value);
    maximum = maximum === undefined ? value : Math.max(maximum, value);
  }

  if (minimum === undefined || maximum === undefined) {
    return [0, 1];
  }

  const padding = (maximum - minimum) * ratio || constantPadding;
  return [minimum - padding, maximum + padding];
}
