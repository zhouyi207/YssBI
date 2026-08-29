import { useEffect, useRef } from 'react';
import { area, axisBottom, axisLeft, line, scaleLinear, select } from 'd3';
import { cn } from '@/lib/utils';
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
import type { XYPoint } from '@/shared/types/visualization/chartModel';

export interface KdeChartProps {
  data: XYPoint[];
  xLabel?: string;
  yLabel?: string;
  color?: string;
  height?: number;
  margin?: ChartMargin;
  xMin?: number;
  className?: string;
}

interface KdeMarkDatum {
  key: 'area' | 'line';
  points: XYPoint[];
}

export function KdeChart({
  data,
  xLabel,
  yLabel = 'Density',
  color,
  height: heightProp,
  margin = DEFAULT_CARTESIAN_MARGIN,
  xMin,
  className,
}: KdeChartProps) {
  const svgRef = useRef<SVGSVGElement>(null);
  const { containerRef, size } = useChartContainerSize();
  const { colors: chartTheme, series: seriesColors } = useChartTheme();
  const plotColor = color ?? seriesColors.primary;

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
      .attr('aria-label', `${yLabel} by ${xLabel ?? 'value'}`);

    if (data.length === 0 || !box) {
      layers.root.attr('display', 'none');
      layers.marks
        .selectAll<SVGPathElement, KdeMarkDatum>('path[data-chart-mark="kde-area"]')
        .data([], mark => mark.key)
        .join('path');
      layers.marks
        .selectAll<SVGPathElement, KdeMarkDatum>('path[data-chart-mark="kde-line"]')
        .data([], mark => mark.key)
        .join('path');
      return;
    }

    layers.root
      .attr('display', null)
      .attr('transform', `translate(${margin.left},${margin.top})`);

    const paddedXDomain = paddedNumericDomain(data.map(point => point.x), 0.06, 1);
    const paddedYDomain = paddedNumericDomain(data.map(point => point.y), 0.06, 0.01);
    const domainMin = xMin == null
      ? paddedXDomain[0]
      : Math.max(xMin, paddedXDomain[0]);
    const yMaximum = Math.max(0, paddedYDomain[1]);
    const xScale = scaleLinear()
      .domain([domainMin, paddedXDomain[1]])
      .range([0, box.plotWidth]);
    const yScale = scaleLinear()
      .domain([0, yMaximum])
      .range([box.plotHeight, 0]);

    updateHorizontalGrid(
      layers.grid,
      yScale.ticks(5),
      value => yScale(value),
      box.plotWidth,
      chartTheme.grid,
    );
    layers.xAxis
      .attr('transform', `translate(0,${box.plotHeight})`)
      .call(axisBottom(xScale).ticks(6).tickSize(-4));
    styleChartAxis(layers.xAxis, chartTheme);
    layers.yAxis.call(axisLeft(yScale).ticks(5).tickSize(-4));
    styleChartAxis(layers.yAxis, chartTheme);
    updateCartesianLabels(
      layers.labels,
      box,
      { x: xLabel, y: yLabel },
      chartTheme.label,
    );

    const densityLine = line<XYPoint>()
      .x(point => xScale(point.x))
      .y(point => yScale(point.y));
    const densityArea = area<XYPoint>()
      .x(point => xScale(point.x))
      .y0(yScale(0))
      .y1(point => yScale(point.y));

    layers.marks
      .selectAll<SVGPathElement, KdeMarkDatum>('path[data-chart-mark="kde-area"]')
      .data([{ key: 'area', points: data }], mark => mark.key)
      .join('path')
      .attr('data-chart-mark', 'kde-area')
      .attr('d', mark => densityArea(mark.points))
      .attr('fill', plotColor)
      .attr('fill-opacity', 0.2);
    layers.marks
      .selectAll<SVGPathElement, KdeMarkDatum>('path[data-chart-mark="kde-line"]')
      .data([{ key: 'line', points: data }], mark => mark.key)
      .join('path')
      .attr('data-chart-mark', 'kde-line')
      .attr('d', mark => densityLine(mark.points))
      .attr('fill', 'none')
      .attr('stroke', plotColor)
      .attr('stroke-width', 2)
      .attr('stroke-linecap', 'round')
      .attr('stroke-linejoin', 'round');
  }, [chartTheme, data, heightProp, margin, plotColor, size, xLabel, xMin, yLabel]);

  return (
    <div
      ref={containerRef}
      className={cn('w-full min-h-0 overflow-hidden', !heightProp && 'h-full', className)}
      style={heightProp ? { height: heightProp } : undefined}
    >
      <svg ref={svgRef} />
    </div>
  );
}
