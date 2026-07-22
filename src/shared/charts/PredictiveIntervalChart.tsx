import { useEffect, useRef } from 'react';
import { area, axisBottom, axisLeft, extent, line, scaleLinear, select } from 'd3';
import { cn } from '@/lib/utils';
import { usePlotContainerSize } from '@/shared/plot/usePlotContainerSize';
import { useChartSeriesColors, useChartThemeColors } from '@/shared/theme/chartTheme';
import { DEFAULT_CHART_MARGIN, type ChartMargin } from './KDEChart';

export interface PredictiveIntervalPoint {
  observation: number;
  observed: number;
  mean: number;
  lower: number;
  upper: number;
}

export interface PredictiveIntervalChartProps {
  data: PredictiveIntervalPoint[];
  xLabel?: string;
  yLabel?: string;
  height?: number;
  margin?: ChartMargin;
  className?: string;
}

export function PredictiveIntervalChart({
  data,
  xLabel = 'observation',
  yLabel = 'value',
  height = 280,
  margin = DEFAULT_CHART_MARGIN,
  className,
}: PredictiveIntervalChartProps) {
  const svgRef = useRef<SVGSVGElement>(null);
  const { containerRef, size } = usePlotContainerSize();
  const chartTheme = useChartThemeColors();
  const seriesColors = useChartSeriesColors();

  useEffect(() => {
    const svg = select(svgRef.current);
    svg.selectAll('*').remove();
    if (data.length === 0 || size.width === 0) return;

    const sorted = [...data]
      .filter(point => [point.observation, point.observed, point.mean, point.lower, point.upper].every(Number.isFinite))
      .sort((left, right) => left.observation - right.observation);
    if (sorted.length === 0) return;

    const plotWidth = Math.max(0, size.width - margin.left - margin.right);
    const plotHeight = Math.max(0, height - margin.top - margin.bottom);
    if (plotWidth === 0 || plotHeight === 0) return;

    const xExtent = extent(sorted, point => point.observation) as [number, number];
    const yExtent = extent(sorted.flatMap(point => [point.lower, point.upper, point.observed])) as [number, number];
    const xPadding = (xExtent[1] - xExtent[0]) * 0.01 || 1;
    const yPadding = (yExtent[1] - yExtent[0]) * 0.06 || 1;
    const xScale = scaleLinear().domain([xExtent[0] - xPadding, xExtent[1] + xPadding]).range([0, plotWidth]);
    const yScale = scaleLinear().domain([yExtent[0] - yPadding, yExtent[1] + yPadding]).range([plotHeight, 0]);

    const root = svg.attr('width', size.width).attr('height', height).attr('role', 'img')
      .attr('aria-label', `${yLabel} posterior predictive interval by ${xLabel}`)
      .append('g').attr('transform', `translate(${margin.left},${margin.top})`);

    root.append('g').selectAll('line').data(yScale.ticks(5)).join('line')
      .attr('x1', 0).attr('x2', plotWidth)
      .attr('y1', value => yScale(value)).attr('y2', value => yScale(value))
      .attr('stroke', chartTheme.grid).attr('stroke-dasharray', '2,3');
    root.append('g').attr('transform', `translate(0,${plotHeight})`).call(axisBottom(xScale).ticks(8).tickSize(-4))
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

    root.append('text').attr('x', plotWidth / 2).attr('y', plotHeight + 32)
      .attr('text-anchor', 'middle').attr('fill', chartTheme.label).attr('font-size', '11px').text(xLabel);
    root.append('text').attr('transform', 'rotate(-90)').attr('x', -plotHeight / 2).attr('y', -42)
      .attr('text-anchor', 'middle').attr('fill', chartTheme.label).attr('font-size', '11px').text(yLabel);

    const intervalArea = area<PredictiveIntervalPoint>()
      .x(point => xScale(point.observation)).y0(point => yScale(point.lower)).y1(point => yScale(point.upper));
    const meanLine = line<PredictiveIntervalPoint>()
      .x(point => xScale(point.observation)).y(point => yScale(point.mean));
    root.append('path').datum(sorted).attr('d', intervalArea).attr('fill', seriesColors.primary).attr('fill-opacity', 0.18);
    root.append('path').datum(sorted).attr('d', meanLine).attr('fill', 'none').attr('stroke', seriesColors.primary)
      .attr('stroke-width', 1.8).attr('stroke-linecap', 'round').attr('stroke-linejoin', 'round');
    root.append('g').selectAll('circle').data(sorted).join('circle')
      .attr('cx', point => xScale(point.observation)).attr('cy', point => yScale(point.observed)).attr('r', 2.4)
      .attr('fill', point => point.observed < point.lower || point.observed > point.upper ? seriesColors.highlight : seriesColors.secondary)
      .attr('stroke', chartTheme.canvas).attr('stroke-width', 0.7)
      .append('title').text(point => `observation ${point.observation}\nobserved: ${point.observed}\nmean: ${point.mean}\n95% interval: [${point.lower}, ${point.upper}]`);
  }, [chartTheme, data, height, margin, seriesColors, size.width, xLabel, yLabel]);

  return (
    <div className={cn('space-y-2', className)}>
      <div ref={containerRef} className="w-full overflow-hidden rounded-md border border-border bg-muted/10" style={{ height }}>
        <svg ref={svgRef} />
      </div>
      <div className="flex flex-wrap gap-4 text-xs text-muted-foreground" aria-label="Chart legend">
        <span className="inline-flex items-center gap-1"><span className="h-2 w-4 rounded-sm" style={{ backgroundColor: seriesColors.primary, opacity: 0.3 }} />95% predictive interval</span>
        <span className="inline-flex items-center gap-1"><span className="h-0.5 w-4" style={{ backgroundColor: seriesColors.primary }} />Predictive mean</span>
        <span className="inline-flex items-center gap-1"><span className="size-2 rounded-full" style={{ backgroundColor: seriesColors.secondary }} />Observed</span>
        <span className="inline-flex items-center gap-1"><span className="size-2 rounded-full" style={{ backgroundColor: seriesColors.highlight }} />Outside interval</span>
      </div>
    </div>
  );
}
