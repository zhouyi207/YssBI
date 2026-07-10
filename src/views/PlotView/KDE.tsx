import React, { useEffect, useRef } from 'react';
import { select, scaleLinear, axisBottom, axisLeft, extent, line, area } from 'd3';
import { useChartThemeColors, useChartSeriesColors } from '@/shared/theme/chartTheme';
import { usePlotContainerSize } from '@/shared/plot/usePlotContainerSize';
import { cn } from '@/lib/utils';
import { DEFAULT_PLOT_MARGIN, plotContainerClass, type PlotMargin } from './plotShellStyles';

export interface KDEPoint {
  x: number;
  y: number;
}

export interface KDEProps {
  /** KDE 数据点：(x, density(x)) */
  data: KDEPoint[];
  /** X 轴标签 */
  xLabel?: string;
  /** Y 轴标签，默认 "Density" */
  yLabel?: string;
  /** 线条颜色，默认 #569cd6 */
  color?: string;
  /** 图表高度，不传则随容器填充 */
  height?: number;
  /** 图表边距 */
  margin?: PlotMargin;
  /** X 轴下界（如 leverage 非负则传 0，避免截断到负轴） */
  xMin?: number;
}

const KDE: React.FC<KDEProps> = ({
  data,
  xLabel,
  yLabel = 'Density',
  color,
  height: heightProp,
  margin = DEFAULT_PLOT_MARGIN,
  xMin: xMinProp,
}) => {
  const svgRef = useRef<SVGSVGElement>(null);
  const { containerRef, size } = usePlotContainerSize();
  const chartTheme = useChartThemeColors();
  const seriesColors = useChartSeriesColors();
  const plotColor = color ?? seriesColors.primary;

  useEffect(() => {
    const svg = select(svgRef.current);
    svg.selectAll('*').remove();

    const container = containerRef.current;
    if (!container || data.length === 0 || size.width === 0 || size.height === 0) return;

    const width = size.width;
    const height = heightProp ?? size.height;
    const w = width - margin.left - margin.right;
    const h = height - margin.top - margin.bottom;

    const xExtent = extent(data, (d) => d.x) as [number, number];
    const yExtent = extent(data, (d) => d.y) as [number, number];
    const xPad = (xExtent[1] - xExtent[0]) * 0.06 || 1;
    const yPad = (yExtent[1] - yExtent[0]) * 0.06 || 0.01;
    const xDomainMin = xMinProp != null ? Math.max(xMinProp, xExtent[0] - xPad) : xExtent[0] - xPad;
    const xScale = scaleLinear()
      .domain([xDomainMin, xExtent[1] + xPad])
      .range([0, w]);
    const yScale = scaleLinear()
      .domain([0, Math.max(0, yExtent[1]) + yPad])
      .range([h, 0]);

    const g = svg
      .attr('width', width)
      .attr('height', height)
      .append('g')
      .attr('transform', `translate(${margin.left},${margin.top})`);

    g.append('g')
      .selectAll('line')
      .data(yScale.ticks(5))
      .join('line')
      .attr('x1', 0)
      .attr('x2', w)
      .attr('y1', (d) => yScale(d))
      .attr('y2', (d) => yScale(d))
      .attr('stroke', chartTheme.grid)
      .attr('stroke-dasharray', '2,3');

    g.append('g')
      .attr('transform', `translate(0,${h})`)
      .call(axisBottom(xScale).ticks(6).tickSize(-4))
      .call((sel) => {
        sel.select('.domain').attr('stroke', chartTheme.axis);
        sel.selectAll('.tick line').attr('stroke', chartTheme.axis);
        sel.selectAll('.tick text').attr('fill', chartTheme.tick).attr('font-size', '10px');
      });

    g.append('g')
      .call(axisLeft(yScale).ticks(5).tickSize(-4))
      .call((sel) => {
        sel.select('.domain').attr('stroke', chartTheme.axis);
        sel.selectAll('.tick line').attr('stroke', chartTheme.axis);
        sel.selectAll('.tick text').attr('fill', chartTheme.tick).attr('font-size', '10px');
      });

    if (xLabel) {
      g.append('text')
        .attr('x', w / 2)
        .attr('y', h + 32)
        .attr('text-anchor', 'middle')
        .attr('fill', chartTheme.label)
        .attr('font-size', '11px')
        .text(xLabel);
    }

    if (yLabel) {
      g.append('text')
        .attr('transform', 'rotate(-90)')
        .attr('x', -h / 2)
        .attr('y', -42)
        .attr('text-anchor', 'middle')
        .attr('fill', chartTheme.label)
        .attr('font-size', '11px')
        .text(yLabel);
    }

    const pathLine = line<KDEPoint>()
      .x((d) => xScale(d.x))
      .y((d) => yScale(d.y));

    const pathArea = area<KDEPoint>()
      .x((d) => xScale(d.x))
      .y0(yScale(0))
      .y1((d) => yScale(d.y));

    g.append('path')
      .datum(data)
      .attr('d', pathArea)
      .attr('fill', plotColor)
      .attr('fill-opacity', 0.2);

    g.append('path')
      .datum(data)
      .attr('d', pathLine)
      .attr('fill', 'none')
      .attr('stroke', plotColor)
      .attr('stroke-width', 2)
      .attr('stroke-linecap', 'round')
      .attr('stroke-linejoin', 'round');
  }, [data, xLabel, yLabel, plotColor, heightProp, margin, xMinProp, size, chartTheme]);

  return (
    <div ref={containerRef} className={cn(plotContainerClass(undefined, heightProp))}>
      <svg ref={svgRef} />
    </div>
  );
};

export default KDE;
