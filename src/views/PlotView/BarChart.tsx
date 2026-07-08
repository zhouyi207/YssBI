import React, { useEffect, useRef } from 'react';
import { select, scaleLinear, scaleBand, axisBottom, axisLeft, max } from 'd3';
import { useChartThemeColors, useChartSeriesColors } from '@/shared/theme/chartTheme';
import { usePlotContainerSize } from '@/shared/plot/usePlotContainerSize';
import {
  attachHoverTooltip,
  type D3Onable,
  PlotTooltipController,
  tooltipTwoLine,
} from '@/shared/plot/d3Tooltip';
import { cn } from '@/lib/utils';
import { COMPACT_PLOT_MARGIN, DEFAULT_PLOT_MARGIN, plotContainerClass, plotTooltipClass } from './plotShellStyles';

export interface BarDatum {
  label: string;
  value: number;
}

export interface BarChartProps {
  data: BarDatum[];
  xLabel?: string;
  yLabel?: string;
  color?: string;
  /** 图表高度，传 0 或不传则自适应容器高度 */
  height?: number;
  margin?: { top: number; right: number; bottom: number; left: number };
  horizontal?: boolean;
  /** 紧凑模式：无轴线，用 tooltip 显示信息 */
  compact?: boolean;
}

const BarChart: React.FC<BarChartProps> = ({
  data,
  xLabel,
  yLabel,
  color,
  height: heightProp,
  margin: marginProp,
  horizontal = false,
  compact = false,
}) => {
  const svgRef = useRef<SVGSVGElement>(null);
  const tooltipRef = useRef<HTMLDivElement>(null);
  const chartTheme = useChartThemeColors();
  const seriesColors = useChartSeriesColors();
  const plotColor = color ?? seriesColors.primary;

  const margin = marginProp ?? (compact ? COMPACT_PLOT_MARGIN : DEFAULT_PLOT_MARGIN);
  const { containerRef, size } = usePlotContainerSize();

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

    const g = svg
      .attr('width', width)
      .attr('height', height)
      .append('g')
      .attr('transform', `translate(${margin.left},${margin.top})`);

    const vMax = (max(data, (d) => d.value) ?? 0) * 1.1;
    const tooltip = new PlotTooltipController(tooltipRef.current, container);

    const bindCompactTooltip = (bars: D3Onable<SVGRectElement, BarDatum>) => {
      if (!compact) return;
      attachHoverTooltip(bars, {
        tooltip,
        getHtml: (d: BarDatum) => tooltipTwoLine(chartTheme, d.label, String(d.value), plotColor),
        onEnter: (el) => select(el).attr('fill-opacity', 1),
        onLeave: (el) => select(el).attr('fill-opacity', 0.75),
      });
    };

    if (horizontal) {
      const yBand = scaleBand()
        .domain(data.map((d) => d.label))
        .range([0, h])
        .padding(compact ? 0.12 : 0.25);

      const xLinear = scaleLinear().domain([0, vMax]).nice().range([0, w]);

      if (!compact) {
        g.append('g')
          .selectAll('line')
          .data(xLinear.ticks(5))
          .join('line')
          .attr('x1', (d) => xLinear(d)).attr('x2', (d) => xLinear(d))
          .attr('y1', 0).attr('y2', h)
          .attr('stroke', chartTheme.grid).attr('stroke-dasharray', '2,3');
      }

      const bars = g.selectAll('rect.bar')
        .data(data)
        .join('rect')
        .attr('class', 'bar')
        .attr('x', 0)
        .attr('y', (d) => yBand(d.label)!)
        .attr('width', (d) => xLinear(d.value))
        .attr('height', yBand.bandwidth())
        .attr('fill', plotColor)
        .attr('fill-opacity', 0.75)
        .attr('rx', 2);

      bindCompactTooltip(bars);

      if (!compact) {
        g.append('g')
          .attr('transform', `translate(0,${h})`)
          .call(axisBottom(xLinear).ticks(6).tickSize(-4))
          .call((sel) => {
            sel.select('.domain').attr('stroke', chartTheme.axis);
            sel.selectAll('.tick line').attr('stroke', chartTheme.axis);
            sel.selectAll('.tick text').attr('fill', chartTheme.tick).attr('font-size', '10px');
          });

        g.append('g')
          .call(axisLeft(yBand).tickSize(0))
          .call((sel) => {
            sel.select('.domain').attr('stroke', chartTheme.axis);
            sel.selectAll('.tick text').attr('fill', chartTheme.tick).attr('font-size', '10px');
          });
      }
    } else {
      const xBand = scaleBand()
        .domain(data.map((d) => d.label))
        .range([0, w])
        .padding(compact ? 0.08 : 0.25);

      const yLinear = scaleLinear().domain([0, vMax]).nice().range([h, 0]);

      if (!compact) {
        g.append('g')
          .selectAll('line')
          .data(yLinear.ticks(5))
          .join('line')
          .attr('x1', 0).attr('x2', w)
          .attr('y1', (d) => yLinear(d)).attr('y2', (d) => yLinear(d))
          .attr('stroke', chartTheme.grid).attr('stroke-dasharray', '2,3');
      }

      const bars = g.selectAll('rect.bar')
        .data(data)
        .join('rect')
        .attr('class', 'bar')
        .attr('x', (d) => xBand(d.label)!)
        .attr('y', (d) => yLinear(d.value))
        .attr('width', xBand.bandwidth())
        .attr('height', (d) => h - yLinear(d.value))
        .attr('fill', plotColor)
        .attr('fill-opacity', 0.75)
        .attr('rx', 2);

      bindCompactTooltip(bars);

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
              .attr('transform', data.length > 8 ? 'rotate(-35)' : '');
          });

        g.append('g')
          .call(axisLeft(yLinear).ticks(5).tickSize(-4))
          .call((sel) => {
            sel.select('.domain').attr('stroke', chartTheme.axis);
            sel.selectAll('.tick line').attr('stroke', chartTheme.axis);
            sel.selectAll('.tick text').attr('fill', chartTheme.tick).attr('font-size', '10px');
          });
      }
    }

    if (!compact) {
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
  }, [data, xLabel, yLabel, plotColor, heightProp, margin, horizontal, compact, size, chartTheme]);

  return (
    <div ref={containerRef} className={cn(plotContainerClass(undefined, heightProp))}>
      <svg ref={svgRef} />
      {compact && (
        <div
          ref={tooltipRef}
          className={plotTooltipClass}
        />
      )}
    </div>
  );
};

export default BarChart;
