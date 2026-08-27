import { useEffect, useRef } from 'react';
import { axisBottom, axisLeft, scaleLinear, select } from 'd3';
import { resolveChartBox } from '@/shared/charts/core/domain';
import {
  joinCartesianLayers,
  styleChartAxis,
} from '@/shared/charts/core/layers';
import { useChartTheme } from '@/shared/charts/core/theme';
import {
  attachMarkTooltip,
  type D3Onable,
  PlotTooltipController,
  tooltipMutedLine,
  tooltipStrongLine,
} from '@/shared/charts/core/tooltip';
import type { ChartMargin } from '@/shared/charts/core/types';
import { useChartContainerSize } from '@/shared/charts/core/useChartContainerSize';

const VAR_STABILITY_MARGIN: ChartMargin = {
  top: 28,
  right: 24,
  bottom: 40,
  left: 48,
};
const DEFAULT_RANGE = 1.3;
const MODULUS_RINGS = [0.2, 0.4, 0.6, 0.8, 1] as const;
const CHART_TOOLTIP_CLASS =
  'pointer-events-none absolute z-10 rounded-md border border-border bg-popover px-3 py-2 opacity-0 shadow-lg transition-opacity duration-100';

export type VarStabilityStatus = 'stable' | 'unstable';
export type VarStabilityValueField = 'real' | 'imaginary' | 'modulus';

export interface VarStabilityPoint {
  re: number;
  im: number;
  modulus: number;
  status: VarStabilityStatus;
}

export interface VarStabilityChartProps {
  data: readonly VarStabilityPoint[];
  xLabel: string;
  yLabel: string;
  ariaLabel: string;
  getPointLabel: (index: number, point: VarStabilityPoint) => string;
  getPointAriaLabel: (point: VarStabilityPoint, index: number) => string;
  modulusLabel: string;
  unstableTooltipLabel: string;
  formatValue?: (value: number, field: VarStabilityValueField) => string;
}

interface IndexedVarStabilityPoint {
  point: VarStabilityPoint;
  index: number;
}

interface ComplexAxisLine {
  axis: 'real' | 'imaginary';
  x1: number;
  x2: number;
  y1: number;
  y2: number;
}

function defaultFormatValue(value: number): string {
  return String(value);
}

export function VarStabilityChart({
  data,
  xLabel,
  yLabel,
  ariaLabel,
  getPointLabel,
  getPointAriaLabel,
  modulusLabel,
  unstableTooltipLabel,
  formatValue = defaultFormatValue,
}: VarStabilityChartProps) {
  const svgRef = useRef<SVGSVGElement>(null);
  const tooltipRef = useRef<HTMLDivElement>(null);
  const { containerRef, size } = useChartContainerSize();
  const { colors: chartTheme, series: seriesColors } = useChartTheme();

  useEffect(() => {
    const svgNode = svgRef.current;
    if (!svgNode) return;

    const svg = select(svgNode);
    const layers = joinCartesianLayers(svg);
    const tooltip = new PlotTooltipController(
      tooltipRef.current,
      containerRef.current,
    );
    const box = resolveChartBox(size.width, size.height, VAR_STABILITY_MARGIN);
    const indexedData = data.map((point, index) => ({ point, index }));

    svg
      .attr('width', size.width)
      .attr('height', size.height)
      .attr('role', 'group')
      .attr('aria-label', ariaLabel);
    tooltip.hide();

    if (!containerRef.current || indexedData.length === 0 || !box) {
      layers.root.attr('display', 'none');
      layers.marks
        .selectAll<SVGCircleElement, IndexedVarStabilityPoint>(
          'circle.var-eigenvalue-point',
        )
        .data([], datum => String(datum.index))
        .join('circle');
      return;
    }

    layers.root
      .attr('display', null)
      .attr(
        'transform',
        `translate(${VAR_STABILITY_MARGIN.left},${VAR_STABILITY_MARGIN.top})`,
      );

    const plotSize = Math.min(box.plotWidth, box.plotHeight);
    const plotOffsetX = (box.plotWidth - plotSize) / 2;
    const plotOffsetY = (box.plotHeight - plotSize) / 2;
    const centerX = box.plotWidth / 2;
    const centerY = box.plotHeight / 2;
    const radiusScale = plotSize / (2 * DEFAULT_RANGE);
    const xScale = scaleLinear()
      .domain([-DEFAULT_RANGE, DEFAULT_RANGE])
      .range([plotOffsetX, plotOffsetX + plotSize]);
    const yScale = scaleLinear()
      .domain([-DEFAULT_RANGE, DEFAULT_RANGE])
      .range([plotOffsetY + plotSize, plotOffsetY]);

    layers.grid
      .selectAll<SVGCircleElement, number>('circle.var-modulus-ring')
      .data(MODULUS_RINGS, radius => radius)
      .join('circle')
      .attr('class', 'var-modulus-ring')
      .attr('data-chart-reference', radius =>
        radius === 1 ? 'unit-circle' : 'modulus-ring')
      .attr('data-chart-value', radius => radius)
      .attr('cx', centerX)
      .attr('cy', centerY)
      .attr('r', radius => radius * radiusScale)
      .attr('fill', 'none')
      .attr('stroke', radius =>
        radius === 1 ? chartTheme.zeroLine : chartTheme.grid)
      .attr('stroke-width', radius => radius === 1 ? 1.5 : 0.8)
      .attr('stroke-dasharray', radius => radius === 1 ? 'none' : '3,3');

    const complexAxes: ComplexAxisLine[] = [
      {
        axis: 'real',
        x1: 0,
        x2: box.plotWidth,
        y1: centerY,
        y2: centerY,
      },
      {
        axis: 'imaginary',
        x1: centerX,
        x2: centerX,
        y1: 0,
        y2: box.plotHeight,
      },
    ];
    layers.grid
      .selectAll<SVGLineElement, ComplexAxisLine>('line.var-complex-axis')
      .data(complexAxes, axis => axis.axis)
      .join('line')
      .attr('class', 'var-complex-axis')
      .attr('data-chart-reference', axis => `${axis.axis}-axis`)
      .attr('x1', axis => axis.x1)
      .attr('x2', axis => axis.x2)
      .attr('y1', axis => axis.y1)
      .attr('y2', axis => axis.y2)
      .attr('stroke', chartTheme.axis)
      .attr('stroke-width', 1);

    layers.xAxis
      .attr('transform', `translate(0,${box.plotHeight})`)
      .call(axisBottom(xScale).ticks(6).tickSize(-4));
    styleChartAxis(layers.xAxis, chartTheme);
    layers.yAxis.call(axisLeft(yScale).ticks(6).tickSize(-4));
    styleChartAxis(layers.yAxis, chartTheme);

    layers.labels
      .selectAll<SVGTextElement, string>('text[data-chart-label="x"]')
      .data([xLabel])
      .join('text')
      .attr('data-chart-label', 'x')
      .attr('x', box.plotWidth / 2)
      .attr('y', -10)
      .attr('text-anchor', 'middle')
      .attr('fill', chartTheme.label)
      .attr('font-size', '11px')
      .text(label => label);
    layers.labels
      .selectAll<SVGTextElement, string>('text[data-chart-label="y"]')
      .data([yLabel])
      .join('text')
      .attr('data-chart-label', 'y')
      .attr('transform', 'rotate(-90)')
      .attr('x', -box.plotHeight / 2)
      .attr('y', -36)
      .attr('text-anchor', 'middle')
      .attr('fill', chartTheme.label)
      .attr('font-size', '11px')
      .text(label => label);

    const points = layers.marks
      .selectAll<SVGCircleElement, IndexedVarStabilityPoint>(
        'circle.var-eigenvalue-point',
      )
      .data(indexedData, datum => String(datum.index))
      .join('circle')
      .attr('class', 'var-eigenvalue-point')
      .attr('data-chart-mark', 'var-eigenvalue')
      .attr('data-status', datum => datum.point.status)
      .attr('cx', datum => xScale(datum.point.re))
      .attr('cy', datum => yScale(datum.point.im))
      .attr('r', 5)
      .attr('fill', datum => datum.point.status === 'unstable'
        ? seriesColors.negative
        : seriesColors.primary)
      .attr('stroke', datum => datum.point.status === 'unstable'
        ? seriesColors.negative
        : seriesColors.primary)
      .attr('stroke-width', 1.5)
      .attr('fill-opacity', 0.9)
      .style('cursor', 'pointer');

    const detachTooltip = attachMarkTooltip(
      points as D3Onable<SVGCircleElement, IndexedVarStabilityPoint>,
      {
        tooltip,
        position: 'anchor',
        getHtml: ({ point, index }) => {
          const real = formatValue(point.re, 'real');
          const imaginary = formatValue(Math.abs(point.im), 'imaginary');
          const complexValue = point.im >= 0
            ? `${real} + ${imaginary}i`
            : `${real} - ${imaginary}i`;
          return tooltipStrongLine(
            getPointLabel(index, point),
            chartTheme,
            { size: 11 },
          )
            + tooltipMutedLine(complexValue, chartTheme, 11)
            + tooltipStrongLine(
              `${modulusLabel}: ${formatValue(point.modulus, 'modulus')}`,
              chartTheme,
              { size: 11 },
            )
            + (point.status === 'unstable'
              ? tooltipStrongLine(
                  unstableTooltipLabel,
                  chartTheme,
                  { size: 10, color: seriesColors.negative },
                )
              : '');
        },
        getAriaLabel: ({ point, index }) => getPointAriaLabel(point, index),
        onEnter: element => select(element)
          .attr('r', 6)
          .attr('stroke-width', 2),
        onLeave: element => select(element)
          .attr('r', 5)
          .attr('stroke-width', 1.5),
      },
    );

    return () => detachTooltip();
  }, [
    ariaLabel,
    chartTheme,
    data,
    formatValue,
    getPointAriaLabel,
    getPointLabel,
    modulusLabel,
    seriesColors.negative,
    seriesColors.primary,
    size,
    unstableTooltipLabel,
    xLabel,
    yLabel,
  ]);

  return (
    <div
      ref={containerRef}
      className="relative min-h-0 w-full flex-1 overflow-hidden rounded-lg border border-border"
      style={{ backgroundColor: chartTheme.canvas }}
    >
      <svg ref={svgRef} className="h-full w-full" />
      <div ref={tooltipRef} className={CHART_TOOLTIP_CLASS} />
    </div>
  );
}
