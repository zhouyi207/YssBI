import { useEffect, useRef } from 'react';
import { axisBottom, axisLeft, line, scaleLinear, select } from 'd3';
import { resolveChartBox } from '@/shared/charts/core/domain';
import {
  joinCartesianLayers,
  styleChartAxis,
  updateCartesianLabels,
} from '@/shared/charts/core/layers';
import { useChartTheme } from '@/shared/charts/core/theme';
import type { ChartMargin } from '@/shared/charts/core/types';
import { useChartContainerSize } from '@/shared/charts/core/useChartContainerSize';

const DID_EVENT_STUDY_MARGIN: ChartMargin = {
  top: 20,
  right: 16,
  bottom: 36,
  left: 52,
};

export interface DidEventStudyPoint {
  rel_time: number;
  coef: number;
  std_err: number;
  ci_low: number;
  ci_high: number;
  is_reference?: boolean;
}

export interface DidEventStudyChartProps {
  points: readonly DidEventStudyPoint[];
  xLabel: string;
  yLabel: string;
  ariaLabel: string;
}

export function DidEventStudyChart({
  points,
  xLabel,
  yLabel,
  ariaLabel,
}: DidEventStudyChartProps) {
  const svgRef = useRef<SVGSVGElement>(null);
  const { containerRef, size } = useChartContainerSize();
  const { colors: chartTheme, series: seriesColors } = useChartTheme();

  useEffect(() => {
    const svgNode = svgRef.current;
    if (!svgNode) return;

    const svg = select(svgNode);
    const layers = joinCartesianLayers(svg);
    const trendLayer = layers.marks
      .selectAll<SVGGElement, string>('g[data-chart-mark-layer="trend"]')
      .data(['trend'], layer => layer)
      .join('g')
      .attr('data-chart-mark-layer', 'trend');
    const intervalLayer = layers.marks
      .selectAll<SVGGElement, string>('g[data-chart-mark-layer="intervals"]')
      .data(['intervals'], layer => layer)
      .join('g')
      .attr('data-chart-mark-layer', 'intervals');
    const pointLayer = layers.marks
      .selectAll<SVGGElement, string>('g[data-chart-mark-layer="points"]')
      .data(['points'], layer => layer)
      .join('g')
      .attr('data-chart-mark-layer', 'points');
    const sorted = [...points].sort((left, right) => left.rel_time - right.rel_time);
    const box = resolveChartBox(
      size.width,
      size.height,
      DID_EVENT_STUDY_MARGIN,
    );

    svg
      .attr('width', size.width)
      .attr('height', size.height)
      .attr('role', 'img')
      .attr('aria-label', ariaLabel);

    if (sorted.length === 0 || !box) {
      layers.root.attr('display', 'none');
      return;
    }

    layers.root
      .attr('display', null)
      .attr(
        'transform',
        `translate(${DID_EVENT_STUDY_MARGIN.left},${DID_EVENT_STUDY_MARGIN.top})`,
      );

    const xValues = sorted.map(point => point.rel_time);
    const xMinimum = Math.min(...xValues);
    const xMaximum = Math.max(...xValues);
    const xPadding = xMaximum === xMinimum
      ? 0.75
      : Math.max(0.5, (xMaximum - xMinimum) * 0.08);
    const xDomain: [number, number] = [
      xMinimum - xPadding,
      xMaximum + xPadding,
    ];

    const yCandidates = [0];
    for (const point of sorted) {
      if (point.is_reference) continue;
      yCandidates.push(point.coef, point.ci_low, point.ci_high);
    }
    let yMinimum = Math.min(...yCandidates);
    let yMaximum = Math.max(...yCandidates);
    if (!Number.isFinite(yMinimum) || !Number.isFinite(yMaximum)) {
      yMinimum = -1;
      yMaximum = 1;
    }
    const yPadding = Math.max(
      (yMaximum - yMinimum) * 0.12,
      0.05 * (Math.max(Math.abs(yMaximum), Math.abs(yMinimum)) + 1e-6),
    );
    const yDomain: [number, number] = [
      yMinimum - yPadding,
      yMaximum + yPadding,
    ];

    const xScale = scaleLinear().domain(xDomain).range([0, box.plotWidth]);
    const yScale = scaleLinear().domain(yDomain).range([box.plotHeight, 0]);

    layers.grid
      .selectAll<SVGRectElement, null>('rect.did-event-study-background')
      .data([null])
      .join('rect')
      .attr('class', 'did-event-study-background')
      .attr('width', box.plotWidth)
      .attr('height', box.plotHeight)
      .attr('fill', chartTheme.canvas)
      .attr('rx', 2);

    layers.grid
      .selectAll<SVGLineElement, number>('line.did-zero-reference')
      .data([0])
      .join('line')
      .attr('class', 'did-zero-reference')
      .attr('data-chart-reference', 'zero')
      .attr('data-chart-value', 0)
      .attr('x1', 0)
      .attr('x2', box.plotWidth)
      .attr('y1', yScale(0))
      .attr('y2', yScale(0))
      .attr('stroke', chartTheme.zeroLine)
      .attr('stroke-dasharray', '4,3');

    layers.grid
      .selectAll<SVGLineElement, number>('line.did-policy-time-reference')
      .data(xDomain[0] <= 0 && xDomain[1] >= 0 ? [0] : [])
      .join('line')
      .attr('class', 'did-policy-time-reference')
      .attr('data-chart-reference', 'policy-time')
      .attr('data-chart-value', 0)
      .attr('x1', xScale(0))
      .attr('x2', xScale(0))
      .attr('y1', 0)
      .attr('y2', box.plotHeight)
      .attr('stroke', chartTheme.label)
      .attr('stroke-dasharray', '3,3')
      .attr('opacity', 0.6);

    layers.xAxis
      .attr('transform', `translate(0,${box.plotHeight})`)
      .call(
        axisBottom(xScale)
          .ticks(Math.min(12, sorted.length + 2))
          .tickFormat(value => String(value)),
      );
    styleChartAxis(layers.xAxis, chartTheme);
    layers.yAxis.call(axisLeft(yScale).ticks(6));
    styleChartAxis(layers.yAxis, chartTheme);
    updateCartesianLabels(
      layers.labels,
      box,
      { x: xLabel, y: yLabel },
      chartTheme.label,
    );

    const trendPoints = sorted.filter(point => !point.is_reference);
    const coefficientLine = line<DidEventStudyPoint>()
      .x(point => xScale(point.rel_time))
      .y(point => yScale(point.coef));
    trendLayer
      .selectAll<SVGPathElement, DidEventStudyPoint[]>('path.did-coefficient-trend')
      .data(trendPoints.length >= 2 ? [trendPoints] : [], () => 'trend')
      .join('path')
      .attr('class', 'did-coefficient-trend')
      .attr('data-chart-mark', 'did-coefficient-trend')
      .attr('d', values => coefficientLine(values))
      .attr('fill', 'none')
      .attr('stroke', seriesColors.primary)
      .attr('stroke-width', 1)
      .attr('stroke-linejoin', 'round')
      .attr('opacity', 0.35);

    const intervalPoints = sorted.filter(point =>
      !point.is_reference
      && point.std_err > 0
      && point.ci_high >= point.ci_low);
    intervalLayer
      .selectAll<SVGLineElement, DidEventStudyPoint>('line.did-confidence-stem')
      .data(intervalPoints, point => String(point.rel_time))
      .join('line')
      .attr('class', 'did-confidence-stem')
      .attr('data-chart-mark', 'did-confidence-interval')
      .attr('data-rel-time', point => point.rel_time)
      .attr('data-ci-low', point => point.ci_low)
      .attr('data-ci-high', point => point.ci_high)
      .attr('x1', point => xScale(point.rel_time))
      .attr('x2', point => xScale(point.rel_time))
      .attr('y1', point => yScale(point.ci_high))
      .attr('y2', point => yScale(point.ci_low))
      .attr('stroke', seriesColors.primary)
      .attr('stroke-width', 1.5)
      .attr('stroke-linecap', 'round');

    const confidenceCap = 4;
    intervalLayer
      .selectAll<SVGLineElement, DidEventStudyPoint>('line.did-confidence-upper-cap')
      .data(intervalPoints, point => String(point.rel_time))
      .join('line')
      .attr('class', 'did-confidence-upper-cap')
      .attr('x1', point => xScale(point.rel_time) - confidenceCap)
      .attr('x2', point => xScale(point.rel_time) + confidenceCap)
      .attr('y1', point => yScale(point.ci_high))
      .attr('y2', point => yScale(point.ci_high))
      .attr('stroke', seriesColors.primary)
      .attr('stroke-width', 1.5);
    intervalLayer
      .selectAll<SVGLineElement, DidEventStudyPoint>('line.did-confidence-lower-cap')
      .data(intervalPoints, point => String(point.rel_time))
      .join('line')
      .attr('class', 'did-confidence-lower-cap')
      .attr('x1', point => xScale(point.rel_time) - confidenceCap)
      .attr('x2', point => xScale(point.rel_time) + confidenceCap)
      .attr('y1', point => yScale(point.ci_low))
      .attr('y2', point => yScale(point.ci_low))
      .attr('stroke', seriesColors.primary)
      .attr('stroke-width', 1.5);

    pointLayer
      .selectAll<SVGCircleElement, DidEventStudyPoint>('circle.did-coefficient-point')
      .data(sorted, point => String(point.rel_time))
      .join('circle')
      .attr('class', 'did-coefficient-point')
      .attr('data-chart-mark', 'did-coefficient')
      .attr('data-rel-time', point => point.rel_time)
      .attr('data-coefficient', point => point.coef)
      .attr('cx', point => xScale(point.rel_time))
      .attr('cy', point => yScale(point.coef))
      .attr('r', point => point.is_reference ? 4.5 : 5)
      .attr('fill', point => point.is_reference ? chartTheme.tick : seriesColors.primary)
      .attr('stroke', chartTheme.axis)
      .attr('stroke-width', 1.2);
  }, [
    ariaLabel,
    chartTheme,
    points,
    seriesColors.primary,
    size,
    xLabel,
    yLabel,
  ]);

  return (
    <div
      ref={containerRef}
      className="mt-3 h-[min(320px,50vh)] min-h-[220px] w-full"
      hidden={points.length === 0}
    >
      <svg ref={svgRef} className="block h-full w-full" />
    </div>
  );
}
