import { useEffect, useRef } from 'react';
import { axisBottom, axisLeft, line, scaleLinear, select } from 'd3';
import {
  paddedNumericDomain,
  resolveChartBox,
} from '@/shared/charts/core/domain';
import {
  joinCartesianLayers,
  styleChartAxis,
  updateCartesianLabels,
  updateHorizontalGrid,
} from '@/shared/charts/core/layers';
import { DEFAULT_CARTESIAN_MARGIN } from '@/shared/charts/core/margins';
import { useChartTheme } from '@/shared/charts/core/theme';
import type { ChartMargin } from '@/shared/charts/core/types';
import { useChartContainerSize } from '@/shared/charts/core/useChartContainerSize';
import type {
  AxisModel,
  XYPoint,
} from '@/shared/types/visualization/chartModel';
import { plotAxisTickFormatter } from './axisFormat';

export interface LineChartProps {
  data: XYPoint[];
  xAxis: AxisModel;
  yAxis: AxisModel;
  showPoints?: boolean;
  color?: string;
  strokeWidth?: number;
  height?: number;
  margin?: ChartMargin;
  className?: string;
}

export function LineChart({
  data,
  xAxis,
  yAxis,
  showPoints = true,
  color,
  strokeWidth = 2,
  height: heightProp,
  margin = DEFAULT_CARTESIAN_MARGIN,
  className,
}: LineChartProps) {
  const svgRef = useRef<SVGSVGElement>(null);
  const { containerRef, size } = useChartContainerSize();
  const { colors: chartTheme, series: seriesColors } = useChartTheme();
  const plotColor = color ?? seriesColors.primary;
  const xLabel = xAxis.label;
  const yLabel = yAxis.label;
  const xValueType = xAxis.valueType;
  const yValueType = yAxis.valueType;

  useEffect(() => {
    const svgNode = svgRef.current;
    if (!svgNode) return;

    const svg = select(svgNode);
    const layers = joinCartesianLayers(svg);
    const width = size.width;
    const height = heightProp ?? size.height;
    const box = resolveChartBox(width, height, margin);

    svg
      .attr('width', width)
      .attr('height', height)
      .attr('role', 'img')
      .attr('aria-label', `${yLabel ?? 'y'} by ${xLabel ?? 'x'}`);

    if (data.length === 0 || !box) {
      layers.root.attr('display', 'none');
      layers.marks
        .selectAll<SVGPathElement, XYPoint[]>('path[data-chart-mark="line-path"]')
        .data([])
        .join('path');
      layers.marks
        .selectAll<SVGCircleElement, XYPoint>('circle[data-chart-mark="line-point"]')
        .data([])
        .join('circle');
      return;
    }

    layers.root
      .attr('display', null)
      .attr('transform', `translate(${margin.left},${margin.top})`);

    const xScale = scaleLinear()
      .domain(paddedNumericDomain(data.map(point => point.x), 0.06, 1))
      .range([0, box.plotWidth]);
    const yScale = scaleLinear()
      .domain(paddedNumericDomain(data.map(point => point.y), 0.06, 1))
      .range([box.plotHeight, 0]);

    updateHorizontalGrid(
      layers.grid,
      yScale.ticks(5),
      value => yScale(value),
      box.plotWidth,
      chartTheme.grid,
    );

    const xAxisGenerator = axisBottom(xScale).ticks(6).tickSize(-4);
    const xTickFormat = plotAxisTickFormatter(xValueType);
    if (xTickFormat) xAxisGenerator.tickFormat(xTickFormat);
    layers.xAxis
      .attr('transform', `translate(0,${box.plotHeight})`)
      .call(xAxisGenerator);
    styleChartAxis(layers.xAxis, chartTheme);

    const yAxisGenerator = axisLeft(yScale).ticks(5).tickSize(-4);
    const yTickFormat = plotAxisTickFormatter(yValueType);
    if (yTickFormat) yAxisGenerator.tickFormat(yTickFormat);
    layers.yAxis.call(yAxisGenerator);
    styleChartAxis(layers.yAxis, chartTheme);
    updateCartesianLabels(
      layers.labels,
      box,
      { x: xLabel, y: yLabel },
      chartTheme.label,
    );

    const pathGenerator = line<XYPoint>()
      .x(point => xScale(point.x))
      .y(point => yScale(point.y));

    layers.marks
      .selectAll<SVGPathElement, XYPoint[]>('path[data-chart-mark="line-path"]')
      .data([data])
      .join('path')
      .attr('data-chart-mark', 'line-path')
      .attr('d', points => pathGenerator(points))
      .attr('fill', 'none')
      .attr('stroke', plotColor)
      .attr('stroke-width', strokeWidth)
      .attr('stroke-linecap', 'round')
      .attr('stroke-linejoin', 'round');

    layers.marks
      .selectAll<SVGCircleElement, XYPoint>('circle[data-chart-mark="line-point"]')
      .data(showPoints ? data : [])
      .join('circle')
      .attr('data-chart-mark', 'line-point')
      .attr('cx', point => xScale(point.x))
      .attr('cy', point => yScale(point.y))
      .attr('r', 3)
      .attr('fill', plotColor)
      .attr('fill-opacity', 0.7)
      .attr('stroke', plotColor)
      .attr('stroke-opacity', 0.3)
      .attr('stroke-width', 1);
  }, [
    chartTheme,
    data,
    heightProp,
    margin,
    plotColor,
    showPoints,
    size,
    strokeWidth,
    xLabel,
    xValueType,
    yLabel,
    yValueType,
  ]);

  return (
    <div
      ref={containerRef}
      className={`relative w-full min-h-0 overflow-hidden${heightProp === undefined ? ' h-full' : ''}${className ? ` ${className}` : ''}`}
      style={heightProp === undefined ? undefined : { height: heightProp }}
    >
      <svg ref={svgRef} />
    </div>
  );
}
