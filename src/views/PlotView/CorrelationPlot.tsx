import React, { useEffect, useRef } from 'react';
import { select, scaleBand, scaleSequential, interpolateRdBu } from 'd3';
import {
  attachMarkTooltip,
  type D3Onable,
  PlotTooltipController,
  tooltipMutedLine,
  tooltipStrongLine,
  tooltipTickLine,
  useChartContainerSize,
  useChartTheme,
  type ChartMargin,
} from '@/shared/charts/core';
import { cn } from '@/lib/utils';
import { plotContainerClass, plotTooltipClass } from './plotShellStyles';

const CORRELATION_PLOT_MARGIN: ChartMargin = {
  top: 40,
  right: 24,
  bottom: 120,
  left: 120,
};

export interface CorrelationPlotProps {
  /** 变量名列表，与 matrix 行列顺序一致 */
  labels: string[];
  /** n×n 相关系数矩阵，值域 [-1, 1] */
  matrix: (number | null)[][];
  /** n×n p 值矩阵，可选 */
  pMatrix?: (number | null)[][];
  /** 图表高度，不传则随容器填充 */
  height?: number;
  /** 图表边距 */
  margin?: ChartMargin;
}

const CorrelationPlot: React.FC<CorrelationPlotProps> = ({
  labels,
  matrix,
  pMatrix,
  height: heightProp,
  margin = CORRELATION_PLOT_MARGIN,
}) => {
  const svgRef = useRef<SVGSVGElement>(null);
  const tooltipRef = useRef<HTMLDivElement>(null);
  const { containerRef, size } = useChartContainerSize();
  const { colors: chartTheme, series: seriesColors } = useChartTheme();

  useEffect(() => {
    const svg = select(svgRef.current);
    svg.selectAll('*').remove();

    const container = containerRef.current;
    const n = labels.length;
    const validSize = size.width > 0 && (size.height > 0 || heightProp != null);
    if (!container || n === 0 || matrix.length === 0 || !validSize) return;
    if (matrix.length !== n || matrix.some((row) => row.length !== n)) return;

    const width = size.width;
    const height = heightProp ?? size.height;
    const w = width - margin.left - margin.right;
    const h = height - margin.top - margin.bottom;

    const leftSpace = 90;
    const bottomSpace = 50;
    const topSpace = 28;
    const rightSpace = 8;
    const availableW = Math.max(0, w - leftSpace - rightSpace);
    const availableH = Math.max(0, h - topSpace - bottomSpace);
    const availableSize = Math.min(availableW, availableH);
    const plotSize = Math.max(availableSize, n * 8);
    const plotW = plotSize;
    const plotH = plotSize;

    const contentCenterX = (plotW - leftSpace) / 2;
    const contentCenterY = (plotH + bottomSpace - topSpace) / 2;
    const offsetX = Math.max(0, w / 2 - contentCenterX);
    const offsetY = Math.max(0, h / 2 - contentCenterY);

    const xScale = scaleBand()
      .domain(labels)
      .range([0, plotW])
      .padding(0.05);

    const yScale = scaleBand()
      .domain(labels)
      .range([0, plotH])
      .padding(0.05);

    const colorScale = scaleSequential(interpolateRdBu)
      .domain([-1, 1]);

    const g = svg
      .attr('width', width)
      .attr('height', height)
      .append('g')
      .attr('transform', `translate(${margin.left + offsetX},${margin.top + offsetY})`);

    const tooltip = new PlotTooltipController(tooltipRef.current, container);

    const cells: { i: number; j: number; value: number; pValue?: number | null }[] = [];
    for (let i = 0; i < n; i++) {
      for (let j = 0; j < n; j++) {
        const v = matrix[i]?.[j];
        if (v != null && !Number.isNaN(v)) {
          const p = pMatrix?.[i]?.[j];
          cells.push({ i, j, value: v, pValue: p });
        }
      }
    }

    const formatP = (p: number | null | undefined): string => {
      if (p == null || Number.isNaN(p)) return '—';
      if (p < 0.001) return 'p < 0.001';
      return `p = ${p.toFixed(3)}`;
    };

    const cellRects = g
      .selectAll('rect.cell')
      .data(cells)
      .join('rect')
      .attr('class', 'cell')
      .attr('x', (d) => xScale(labels[d.j]) ?? 0)
      .attr('y', (d) => yScale(labels[d.i]) ?? 0)
      .attr('width', xScale.bandwidth())
      .attr('height', yScale.bandwidth())
      .attr('fill', (d) => colorScale(d.value))
      .attr('stroke', chartTheme.grid)
      .attr('stroke-width', 0.5)
      .attr('rx', 2);

    const detachTooltip = attachMarkTooltip(
      cellRects as D3Onable<
        SVGRectElement,
        { i: number; j: number; value: number; pValue?: number | null }
      >,
      {
        tooltip,
        cursorOffset: { x: 12, y: -10 },
        getHtml: (d) =>
          tooltipMutedLine(`${labels[d.i]} × ${labels[d.j]}`, chartTheme) +
          tooltipStrongLine(`r = ${d.value.toFixed(3)}`, chartTheme) +
          tooltipTickLine(formatP(d.pValue), chartTheme),
        getAriaLabel: (d) =>
          `Correlation between ${labels[d.i]} and ${labels[d.j]}, r ${d.value.toFixed(3)}, ${formatP(d.pValue)}`,
        onEnter: (el) =>
          select(el).attr('stroke', seriesColors.primary).attr('stroke-width', 2),
        onLeave: (el) =>
          select(el).attr('stroke', chartTheme.grid).attr('stroke-width', 0.5),
      },
    );

    // X 轴标签（旋转 -45° 避免重叠）
    const xAxisG = g.append('g').attr('transform', `translate(0,${plotH})`);
    xAxisG
      .selectAll('text')
      .data(labels)
      .join('text')
      .attr('x', (name) => (xScale(name) ?? 0) + xScale.bandwidth() / 2)
      .attr('y', 16)
      .attr('text-anchor', 'end')
      .attr('transform', (name) => {
        const x = (xScale(name) ?? 0) + xScale.bandwidth() / 2;
        return `rotate(-45 ${x} 16)`;
      })
      .attr('fill', chartTheme.tick)
      .attr('font-size', '10px')
      .text((name) => (name.length > 12 ? name.slice(0, 10) + '…' : name));

    // Y 轴标签
    g.append('g')
      .selectAll('text')
      .data(labels)
      .join('text')
      .attr('x', -10)
      .attr('y', (name) => (yScale(name) ?? 0) + yScale.bandwidth() / 2)
      .attr('text-anchor', 'end')
      .attr('dominant-baseline', 'middle')
      .attr('fill', chartTheme.tick)
      .attr('font-size', '10px')
      .text((name) => (name.length > 12 ? name.slice(0, 10) + '…' : name));

    // 颜色图例（画布左下角，竖条）
    const legendW = 8;
    const legendH = 100;
    const legendX = margin.left + 8;
    const legendY = height - margin.bottom - legendH - 20;
    const defs = svg.append('defs');
    const gradientId = 'corr-gradient';
    defs
      .append('linearGradient')
      .attr('id', gradientId)
      .attr('x1', 0)
      .attr('x2', 0)
      .attr('y1', 1)
      .attr('y2', 0)
      .selectAll('stop')
      .data([
        { offset: '0%', color: colorScale(-1) },
        { offset: '50%', color: colorScale(0) },
        { offset: '100%', color: colorScale(1) },
      ])
      .join('stop')
      .attr('offset', (d) => d.offset)
      .attr('stop-color', (d) => d.color);
    svg
      .append('rect')
      .attr('x', legendX)
      .attr('y', legendY)
      .attr('width', legendW)
      .attr('height', legendH)
      .attr('fill', `url(#${gradientId})`)
      .attr('rx', 2);
    svg
      .append('text')
      .attr('x', legendX + legendW + 6)
      .attr('y', legendY + legendH)
      .attr('text-anchor', 'start')
      .attr('dominant-baseline', 'middle')
      .attr('fill', chartTheme.label)
      .attr('font-size', '9px')
      .text('-1');
    svg
      .append('text')
      .attr('x', legendX + legendW + 6)
      .attr('y', legendY)
      .attr('text-anchor', 'start')
      .attr('dominant-baseline', 'middle')
      .attr('fill', chartTheme.label)
      .attr('font-size', '9px')
      .text('1');
    return detachTooltip;
  }, [labels, matrix, pMatrix, heightProp, margin, size, chartTheme, seriesColors.primary]);

  return (
    <div ref={containerRef} className={cn(plotContainerClass(), 'relative')}>
      <svg ref={svgRef} />
      <div ref={tooltipRef} className={plotTooltipClass} />
    </div>
  );
};

export default CorrelationPlot;
