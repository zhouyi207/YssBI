import React, { useEffect, useRef } from 'react';
import { select, scaleLinear, scaleBand, axisBottom, axisLeft, max } from 'd3';
import {
  attachMarkTooltip,
  DEFAULT_CARTESIAN_MARGIN,
  type D3Onable,
  PlotTooltipController,
  tooltipTwoLine,
  useChartContainerSize,
  useChartTheme,
  type ChartMargin,
} from '@/shared/charts/core';
import { cn } from '@/lib/utils';
import { plotContainerClass, plotTooltipClass } from './plotShellStyles';

const COMPACT_HISTOGRAM_MARGIN: ChartMargin = {
  top: 4,
  right: 4,
  bottom: 4,
  left: 4,
};

export interface HistogramBin {
  label: string;
  count: number;
}

export interface HistogramProps {
  data: HistogramBin[];
  xLabel?: string;
  yLabel?: string;
  color?: string;
  /** 图表高度，传 0 或不传则自适应容器高度 */
  height?: number;
  margin?: ChartMargin;
  /** 紧凑模式：无轴线，用 tooltip 显示信息 */
  compact?: boolean;
  /** 嵌入编辑器工作表：无边框、无圆角、填满容器 */
  embedded?: boolean;
}

const Histogram: React.FC<HistogramProps> = ({
  data,
  xLabel,
  yLabel = 'Frequency',
  color,
  height: heightProp,
  margin: marginProp,
  compact = false,
  embedded = false,
}) => {
  const svgRef = useRef<SVGSVGElement>(null);
  const tooltipRef = useRef<HTMLDivElement>(null);
  const { colors: chartTheme, series: seriesColors } = useChartTheme();
  const plotColor = color ?? seriesColors.primary;

  const margin = marginProp ?? (compact ? COMPACT_HISTOGRAM_MARGIN : DEFAULT_CARTESIAN_MARGIN);
  const { containerRef, size } = useChartContainerSize();

  useEffect(() => {
    const svg = select(svgRef.current);
    svg.selectAll('*').remove();

    const container = containerRef.current;
    if (!container || data.length === 0 || size.width === 0 || (size.height === 0 && !heightProp)) return;

    const width = size.width;
    const height = heightProp ?? (size.height || 280);
    const w = width - margin.left - margin.right;
    const h = height - margin.top - margin.bottom;
    if (w <= 0 || h <= 0) return;

    const xBand = scaleBand()
      .domain(data.map((d) => d.label))
      .range([0, w])
      .padding(compact ? 0.04 : 0.08);

    const yMax = (max(data, (d) => d.count) ?? 0) * 1.1;
    const yScale = scaleLinear().domain([0, yMax]).nice().range([h, 0]);

    const g = svg
      .attr('width', width)
      .attr('height', height)
      .append('g')
      .attr('transform', `translate(${margin.left},${margin.top})`);

    if (!compact) {
      g.append('g')
        .selectAll('line')
        .data(yScale.ticks(5))
        .join('line')
        .attr('x1', 0).attr('x2', w)
        .attr('y1', (d) => yScale(d)).attr('y2', (d) => yScale(d))
        .attr('stroke', chartTheme.grid).attr('stroke-dasharray', '2,3');
    }

    const tooltip = new PlotTooltipController(tooltipRef.current, container);

    const bars = g.selectAll('rect.bar')
      .data(data)
      .join('rect')
      .attr('class', 'bar')
      .attr('x', (d) => xBand(d.label)!)
      .attr('y', (d) => yScale(d.count))
      .attr('width', xBand.bandwidth())
      .attr('height', (d) => h - yScale(d.count))
      .attr('fill', plotColor)
      .attr('fill-opacity', 0.75)
      .attr('stroke', plotColor)
      .attr('stroke-opacity', 0.4)
      .attr('stroke-width', 0.5)
      .attr('rx', compact ? 1 : 0);

    if (compact) {
      attachMarkTooltip(bars as D3Onable<SVGRectElement, HistogramBin>, {
        tooltip,
        getHtml: (d) => tooltipTwoLine(chartTheme, d.label, String(d.count), plotColor),
        getAriaLabel: (d) => `Histogram bin ${d.label}, ${yLabel} ${d.count}`,
        onEnter: (el) => select(el).attr('fill-opacity', 1),
        onLeave: (el) => select(el).attr('fill-opacity', 0.75),
      });
    }

    if (!compact) {
      g.append('g')
        .attr('transform', `translate(0,${h})`)
        .call(axisBottom(xBand).tickSize(-4))
        .call((sel) => {
          sel.select('.domain').attr('stroke', chartTheme.axis);
          sel.selectAll('.tick line').attr('stroke', chartTheme.axis);
          sel.selectAll('.tick text')
            .attr('fill', chartTheme.tick).attr('font-size', '10px')
            .attr('text-anchor', 'end')
            .attr('transform', data.length > 6 ? 'rotate(-40)' : '');
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
    }
  }, [data, xLabel, yLabel, plotColor, heightProp, margin, compact, size, chartTheme]);

  return (
    <div ref={containerRef} className={cn('relative', plotContainerClass(embedded, heightProp))}>
      <svg ref={svgRef} />
      {compact && <div ref={tooltipRef} className={plotTooltipClass} />}
    </div>
  );
};

export default Histogram;
