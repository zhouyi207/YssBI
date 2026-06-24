import React, { useEffect, useRef, useState } from 'react';
import { select, scaleLinear, scaleBand, axisBottom, axisLeft, max } from 'd3';
import { useChartThemeColors } from '@/shared/theme/chartTheme';

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

const DEFAULT_MARGIN = { top: 20, right: 24, bottom: 40, left: 56 };
const COMPACT_MARGIN = { top: 4, right: 4, bottom: 4, left: 4 };
const DEFAULT_COLOR = '#569cd6';

const BarChart: React.FC<BarChartProps> = ({
  data,
  xLabel,
  yLabel,
  color = DEFAULT_COLOR,
  height: heightProp,
  margin: marginProp,
  horizontal = false,
  compact = false,
}) => {
  const containerRef = useRef<HTMLDivElement>(null);
  const svgRef = useRef<SVGSVGElement>(null);
  const tooltipRef = useRef<HTMLDivElement>(null);
  const [size, setSize] = useState({ width: 0, height: 0 });
  const chartTheme = useChartThemeColors();

  const margin = marginProp ?? (compact ? COMPACT_MARGIN : DEFAULT_MARGIN);

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;
    const ro = new ResizeObserver(() => {
      setSize({ width: container.clientWidth, height: container.clientHeight });
    });
    ro.observe(container);
    setSize({ width: container.clientWidth, height: container.clientHeight });
    return () => ro.disconnect();
  }, []);

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
    const tooltip = select(tooltipRef.current);

    const attachTooltip = (sel: any) => {
      if (!compact) return;
      sel
        .on('mouseenter', function (_event: any, d: any) {
          select(this).attr('fill-opacity', 1);
          tooltip
            .style('opacity', '1')
            .html(`<div style="font-size:10px;color:${chartTheme.tooltipFg}">${d.label}</div><div style="font-size:11px;font-weight:600;color:${color}">${d.value}</div>`);
        })
        .on('mousemove', function (event: any) {
          const rect = container!.getBoundingClientRect();
          tooltip
            .style('left', `${event.clientX - rect.left + 8}px`)
            .style('top', `${event.clientY - rect.top - 36}px`);
        })
        .on('mouseleave', function () {
          select(this).attr('fill-opacity', 0.75);
          tooltip.style('opacity', '0');
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
        .attr('fill', color)
        .attr('fill-opacity', 0.75)
        .attr('rx', 2);

      attachTooltip(bars);

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
        .attr('fill', color)
        .attr('fill-opacity', 0.75)
        .attr('rx', 2);

      attachTooltip(bars);

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
  }, [data, xLabel, yLabel, color, heightProp, margin, horizontal, compact, size, chartTheme]);

  return (
    <div ref={containerRef} className={`relative rounded-lg border border-gray-800/50 bg-[#13151a] overflow-hidden ${!heightProp ? 'w-full h-full min-h-0' : ''}`}>
      <svg ref={svgRef} />
      {compact && (
        <div
          ref={tooltipRef}
          className="absolute pointer-events-none rounded px-2 py-1 bg-[#1e2028] border border-gray-700 shadow-lg opacity-0 transition-opacity duration-100 z-10 whitespace-nowrap"
        />
      )}
    </div>
  );
};

export default BarChart;
