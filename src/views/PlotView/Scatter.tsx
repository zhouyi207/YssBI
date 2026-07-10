import React, { useEffect, useRef } from 'react';
import { select, scaleLinear, axisBottom, axisLeft, extent } from 'd3';
import { useChartThemeColors, useChartSeriesColors } from '@/shared/theme/chartTheme';
import { plotAxisTickFormatter } from '@/shared/plot/plotTime';
import { usePlotContainerSize } from '@/shared/plot/usePlotContainerSize';
import { cn } from '@/lib/utils';
import { DEFAULT_PLOT_MARGIN, plotContainerClass, type PlotMargin } from './plotShellStyles';

export interface ScatterPoint {
  x: number;
  y: number;
}

export interface ScatterProps {
  data: ScatterPoint[];
  /** X 轴标签 */
  xLabel?: string;
  /** Y 轴标签 */
  yLabel?: string;
  /** X 轴格式: "date"=天数转日期, "datetime"=微秒转日期时间, "number"=普通数值 */
  xFormat?: 'date' | 'datetime' | 'number';
  /** Y 轴格式 */
  yFormat?: 'date' | 'datetime' | 'number';
  /** 散点颜色，默认 #569cd6 */
  color?: string;
  /** 散点半径，默认 3 */
  radius?: number;
  /** 图表高度，不传则随容器填充 */
  height?: number;
  /** 图表边距 */
  margin?: PlotMargin;
  /** Y 轴是否关于 0 对称（如残差图），默认 false */
  symmetricY?: boolean;
  /** 是否绘制 y=0 参考线，默认 false */
  zeroLine?: boolean;
  /** 高亮点的索引（如异常值），使用 highlightColor 绘制 */
  highlightIndices?: Set<number>;
  /** 高亮点颜色，默认 #ef4444 */
  highlightColor?: string;
  /** 嵌入编辑器工作表：无边框、无圆角、填满容器 */
  embedded?: boolean;
}

const Scatter: React.FC<ScatterProps> = ({
  data,
  xLabel,
  yLabel,
  xFormat = 'number',
  yFormat = 'number',
  color,
  radius = 3,
  height: heightProp,
  margin = DEFAULT_PLOT_MARGIN,
  symmetricY = false,
  zeroLine = false,
  highlightIndices,
  highlightColor,
  embedded = false,
}) => {
  const svgRef = useRef<SVGSVGElement>(null);
  const { containerRef, size } = usePlotContainerSize();
  const chartTheme = useChartThemeColors();
  const seriesColors = useChartSeriesColors();
  const plotColor = color ?? seriesColors.primary;
  const plotHighlightColor = highlightColor ?? seriesColors.highlight;

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
    const xPad = (xExtent[1] - xExtent[0]) * 0.06 || 1;
    const xScale = scaleLinear().domain([xExtent[0] - xPad, xExtent[1] + xPad]).range([0, w]);

    let yDomain: [number, number];
    if (symmetricY) {
      const yExtent = extent(data, (d) => d.y) as [number, number];
      const yMax = Math.max(Math.abs(yExtent[0]), Math.abs(yExtent[1])) * 1.15;
      yDomain = [-yMax, yMax];
    } else {
      const yExtent = extent(data, (d) => d.y) as [number, number];
      const yPad = (yExtent[1] - yExtent[0]) * 0.06 || 1;
      yDomain = [yExtent[0] - yPad, yExtent[1] + yPad];
    }
    const yScale = scaleLinear().domain(yDomain).range([h, 0]);

    const g = svg
      .attr('width', width)
      .attr('height', height)
      .append('g')
      .attr('transform', `translate(${margin.left},${margin.top})`);

    // grid lines
    g.append('g')
      .selectAll('line')
      .data(yScale.ticks(5))
      .join('line')
      .attr('x1', 0).attr('x2', w)
      .attr('y1', (d) => yScale(d)).attr('y2', (d) => yScale(d))
      .attr('stroke', chartTheme.grid).attr('stroke-dasharray', '2,3');

    // zero reference line
    if (zeroLine) {
      g.append('line')
        .attr('x1', 0).attr('x2', w)
        .attr('y1', yScale(0)).attr('y2', yScale(0))
        .attr('stroke', chartTheme.zeroLine).attr('stroke-width', 1);
    }

    // x axis
    const xAxis = axisBottom(xScale).ticks(6).tickSize(-4);
    const xTickFormat = plotAxisTickFormatter(xFormat);
    if (xTickFormat) xAxis.tickFormat(xTickFormat);
    g.append('g')
      .attr('transform', `translate(0,${h})`)
      .call(xAxis)
      .call((sel) => {
        sel.select('.domain').attr('stroke', chartTheme.axis);
        sel.selectAll('.tick line').attr('stroke', chartTheme.axis);
        sel.selectAll('.tick text').attr('fill', chartTheme.tick).attr('font-size', '10px');
      });

    // y axis
    const yAxis = axisLeft(yScale).ticks(5).tickSize(-4);
    const yTickFormat = plotAxisTickFormatter(yFormat);
    if (yTickFormat) yAxis.tickFormat(yTickFormat);
    g.append('g')
      .call(yAxis)
      .call((sel) => {
        sel.select('.domain').attr('stroke', chartTheme.axis);
        sel.selectAll('.tick line').attr('stroke', chartTheme.axis);
        sel.selectAll('.tick text').attr('fill', chartTheme.tick).attr('font-size', '10px');
      });

    if (xLabel) {
      g.append('text')
        .attr('x', w / 2).attr('y', h + 32)
        .attr('text-anchor', 'middle')
        .attr('fill', chartTheme.label).attr('font-size', '11px')
        .text(xLabel);
    }

    if (yLabel) {
      g.append('text')
        .attr('transform', 'rotate(-90)')
        .attr('x', -h / 2).attr('y', -42)
        .attr('text-anchor', 'middle')
        .attr('fill', chartTheme.label).attr('font-size', '11px')
        .text(yLabel);
    }

    // points
    g.selectAll('circle')
      .data(data)
      .join('circle')
      .attr('cx', (d) => xScale(d.x))
      .attr('cy', (d) => yScale(d.y))
      .attr('r', radius)
      .attr('fill', (_, i) => (highlightIndices?.has(i) ? plotHighlightColor : plotColor))
      .attr('fill-opacity', 0.7)
      .attr('stroke', (_, i) => (highlightIndices?.has(i) ? plotHighlightColor : plotColor))
      .attr('stroke-opacity', 0.3)
      .attr('stroke-width', 1);
  }, [data, xLabel, yLabel, xFormat, yFormat, plotColor, radius, heightProp, margin, symmetricY, zeroLine, highlightIndices, plotHighlightColor, size, chartTheme]);

  return (
    <div ref={containerRef} className={cn(plotContainerClass(embedded, heightProp))}>
      <svg ref={svgRef} />
    </div>
  );
};

export default Scatter;
