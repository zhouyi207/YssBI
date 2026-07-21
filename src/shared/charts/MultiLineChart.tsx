import { useEffect, useMemo, useRef } from 'react';
import { axisBottom, axisLeft, extent, line, scaleLinear, select } from 'd3';
import { cn } from '@/lib/utils';
import { usePlotContainerSize } from '@/shared/plot/usePlotContainerSize';
import { useChartSeriesColors, useChartThemeColors } from '@/shared/theme/chartTheme';
import { DEFAULT_CHART_MARGIN, type ChartMargin } from './KDEChart';

export interface MultiLinePoint {
  x: number;
  y: number;
}

export interface MultiLineSeries {
  id: string;
  label: string;
  points: MultiLinePoint[];
  color?: string;
}

export interface MultiLineChartProps {
  series: MultiLineSeries[];
  xLabel?: string;
  yLabel?: string;
  height?: number;
  margin?: ChartMargin;
  xDomain?: [number, number];
  yDomain?: [number, number];
  showLegend?: boolean;
  className?: string;
}

const FALLBACK_SERIES_COLORS = ['#61afef', '#e06c75', '#98c379', '#e5c07b', '#c678dd', '#56b6c2'];

function paddedDomain(values: number[]): [number, number] {
  const domain = extent(values.filter(Number.isFinite)) as [number | undefined, number | undefined];
  if (domain[0] == null || domain[1] == null) return [0, 1];
  if (domain[0] === domain[1]) return [domain[0] - 1, domain[1] + 1];
  const padding = (domain[1] - domain[0]) * 0.04;
  return [domain[0] - padding, domain[1] + padding];
}

export function MultiLineChart({
  series,
  xLabel,
  yLabel,
  height = 224,
  margin = DEFAULT_CHART_MARGIN,
  xDomain,
  yDomain,
  showLegend = true,
  className,
}: MultiLineChartProps) {
  const svgRef = useRef<SVGSVGElement>(null);
  const { containerRef, size } = usePlotContainerSize();
  const chartTheme = useChartThemeColors();
  const themeSeries = useChartSeriesColors();
  const visibleSeries = useMemo(() => series.filter(item => item.points.length > 0), [series]);
  const colors = useMemo(
    () => [themeSeries.primary, themeSeries.negative, themeSeries.secondary, ...FALLBACK_SERIES_COLORS],
    [themeSeries],
  );

  useEffect(() => {
    const svg = select(svgRef.current);
    svg.selectAll('*').remove();
    if (!containerRef.current || visibleSeries.length === 0 || size.width === 0) return;

    const width = size.width;
    const plotWidth = Math.max(0, width - margin.left - margin.right);
    const plotHeight = Math.max(0, height - margin.top - margin.bottom);
    if (plotWidth === 0 || plotHeight === 0) return;

    const points = visibleSeries.flatMap(item => item.points);
    const resolvedXDomain = xDomain ?? paddedDomain(points.map(point => point.x));
    const resolvedYDomain = yDomain ?? paddedDomain(points.map(point => point.y));
    const xScale = scaleLinear().domain(resolvedXDomain).range([0, plotWidth]);
    const yScale = scaleLinear().domain(resolvedYDomain).range([plotHeight, 0]);
    const root = svg.attr('width', width).attr('height', height).attr('role', 'img')
      .attr('aria-label', `${yLabel ?? 'value'} by ${xLabel ?? 'x'}`)
      .append('g').attr('transform', `translate(${margin.left},${margin.top})`);

    root.append('g').selectAll('line').data(yScale.ticks(5)).join('line')
      .attr('x1', 0).attr('x2', plotWidth)
      .attr('y1', value => yScale(value)).attr('y2', value => yScale(value))
      .attr('stroke', chartTheme.grid).attr('stroke-dasharray', '2,3');
    root.append('g').attr('transform', `translate(0,${plotHeight})`).call(axisBottom(xScale).ticks(6).tickSize(-4))
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

    const path = line<MultiLinePoint>().defined(point => Number.isFinite(point.x) && Number.isFinite(point.y))
      .x(point => xScale(point.x)).y(point => yScale(point.y));
    visibleSeries.forEach((item, index) => {
      root.append('path').datum(item.points).attr('d', path).attr('fill', 'none')
        .attr('stroke', item.color ?? colors[index % colors.length]).attr('stroke-width', 1.8)
        .attr('stroke-linecap', 'round').attr('stroke-linejoin', 'round');
    });
  }, [chartTheme, colors, containerRef, height, margin, size.width, visibleSeries, xDomain, xLabel, yDomain, yLabel]);

  if (visibleSeries.length === 0) return null;

  return (
    <div className={cn('w-full space-y-2', className)}>
      <div ref={containerRef} className="w-full overflow-hidden rounded-md border border-border bg-muted/10" style={{ height }}>
        <svg ref={svgRef} />
      </div>
      {showLegend ? (
        <div className="flex flex-wrap gap-3 text-xs text-muted-foreground" aria-label="Chart legend">
          {visibleSeries.map((item, index) => (
            <span key={item.id} className="inline-flex items-center gap-1">
              <span className="size-2 rounded-full" style={{ backgroundColor: item.color ?? colors[index % colors.length] }} />
              {item.label}
            </span>
          ))}
        </div>
      ) : null}
    </div>
  );
}
