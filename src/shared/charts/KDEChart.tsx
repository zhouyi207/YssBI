import { useEffect, useRef } from 'react';
import { area, axisBottom, axisLeft, extent, line, scaleLinear, select } from 'd3';
import { cn } from '@/lib/utils';
import { usePlotContainerSize } from '@/shared/plot/usePlotContainerSize';
import { useChartSeriesColors, useChartThemeColors } from '@/shared/theme/chartTheme';
import type { PlotPointDTO } from '@/shared/types/dto/plotPayload';

export type KDEPoint = PlotPointDTO;

export interface ChartMargin {
  top: number;
  right: number;
  bottom: number;
  left: number;
}

export const DEFAULT_CHART_MARGIN: ChartMargin = { top: 20, right: 24, bottom: 40, left: 56 };

export interface KDEChartProps {
  data: KDEPoint[];
  xLabel?: string;
  yLabel?: string;
  color?: string;
  height?: number;
  margin?: ChartMargin;
  xMin?: number;
  className?: string;
}

export function KDEChart({
  data,
  xLabel,
  yLabel = 'Density',
  color,
  height: heightProp,
  margin = DEFAULT_CHART_MARGIN,
  xMin,
  className,
}: KDEChartProps) {
  const svgRef = useRef<SVGSVGElement>(null);
  const { containerRef, size } = usePlotContainerSize();
  const chartTheme = useChartThemeColors();
  const seriesColors = useChartSeriesColors();
  const plotColor = color ?? seriesColors.primary;

  useEffect(() => {
    const svg = select(svgRef.current);
    svg.selectAll('*').remove();
    if (!containerRef.current || data.length === 0 || size.width === 0 || size.height === 0) return;

    const width = size.width;
    const height = heightProp ?? size.height;
    const plotWidth = Math.max(0, width - margin.left - margin.right);
    const plotHeight = Math.max(0, height - margin.top - margin.bottom);
    if (plotWidth === 0 || plotHeight === 0) return;

    const xExtent = extent(data, point => point.x) as [number, number];
    const yExtent = extent(data, point => point.y) as [number, number];
    const xPad = (xExtent[1] - xExtent[0]) * 0.06 || 1;
    const yPad = (yExtent[1] - yExtent[0]) * 0.06 || 0.01;
    const domainMin = xMin == null ? xExtent[0] - xPad : Math.max(xMin, xExtent[0] - xPad);
    const xScale = scaleLinear().domain([domainMin, xExtent[1] + xPad]).range([0, plotWidth]);
    const yScale = scaleLinear().domain([0, Math.max(0, yExtent[1]) + yPad]).range([plotHeight, 0]);

    const root = svg
      .attr('width', width)
      .attr('height', height)
      .attr('role', 'img')
      .attr('aria-label', `${yLabel} by ${xLabel ?? 'value'}`)
      .append('g')
      .attr('transform', `translate(${margin.left},${margin.top})`);

    root.append('g').selectAll('line').data(yScale.ticks(5)).join('line')
      .attr('x1', 0).attr('x2', plotWidth)
      .attr('y1', value => yScale(value)).attr('y2', value => yScale(value))
      .attr('stroke', chartTheme.grid).attr('stroke-dasharray', '2,3');

    root.append('g').attr('transform', `translate(0,${plotHeight})`)
      .call(axisBottom(xScale).ticks(6).tickSize(-4))
      .call(selection => {
        selection.select('.domain').attr('stroke', chartTheme.axis);
        selection.selectAll('.tick line').attr('stroke', chartTheme.axis);
        selection.selectAll('.tick text').attr('fill', chartTheme.tick).attr('font-size', '10px');
      });
    root.append('g').call(axisLeft(yScale).ticks(5).tickSize(-4)).call(selection => {
      selection.select('.domain').attr('stroke', chartTheme.axis);
      selection.selectAll('.tick line').attr('stroke', chartTheme.axis);
      selection.selectAll('.tick text').attr('fill', chartTheme.tick).attr('font-size', '10px');
    });

    if (xLabel) root.append('text').attr('x', plotWidth / 2).attr('y', plotHeight + 32)
      .attr('text-anchor', 'middle').attr('fill', chartTheme.label).attr('font-size', '11px').text(xLabel);
    if (yLabel) root.append('text').attr('transform', 'rotate(-90)').attr('x', -plotHeight / 2).attr('y', -42)
      .attr('text-anchor', 'middle').attr('fill', chartTheme.label).attr('font-size', '11px').text(yLabel);

    const densityLine = line<KDEPoint>().x(point => xScale(point.x)).y(point => yScale(point.y));
    const densityArea = area<KDEPoint>().x(point => xScale(point.x)).y0(yScale(0)).y1(point => yScale(point.y));
    root.append('path').datum(data).attr('d', densityArea).attr('fill', plotColor).attr('fill-opacity', 0.2);
    root.append('path').datum(data).attr('d', densityLine).attr('fill', 'none').attr('stroke', plotColor)
      .attr('stroke-width', 2).attr('stroke-linecap', 'round').attr('stroke-linejoin', 'round');
  }, [chartTheme, containerRef, data, heightProp, margin, plotColor, size, xLabel, xMin, yLabel]);

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
