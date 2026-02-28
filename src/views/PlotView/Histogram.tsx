import React, { useEffect, useRef } from 'react';
import { select, scaleLinear, scaleBand, axisBottom, axisLeft, max } from 'd3';

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
  margin?: { top: number; right: number; bottom: number; left: number };
  /** 紧凑模式：无轴线，用 tooltip 显示信息 */
  compact?: boolean;
}

const DEFAULT_MARGIN = { top: 20, right: 24, bottom: 40, left: 56 };
const COMPACT_MARGIN = { top: 4, right: 4, bottom: 4, left: 4 };
const DEFAULT_COLOR = '#569cd6';

const Histogram: React.FC<HistogramProps> = ({
  data,
  xLabel,
  yLabel = 'Frequency',
  color = DEFAULT_COLOR,
  height: heightProp,
  margin: marginProp,
  compact = false,
}) => {
  const containerRef = useRef<HTMLDivElement>(null);
  const svgRef = useRef<SVGSVGElement>(null);
  const tooltipRef = useRef<HTMLDivElement>(null);

  const margin = marginProp ?? (compact ? COMPACT_MARGIN : DEFAULT_MARGIN);

  useEffect(() => {
    const svg = select(svgRef.current);
    svg.selectAll('*').remove();

    const container = containerRef.current;
    if (!container || data.length === 0) return;

    const width = container.clientWidth;
    const height = heightProp || container.clientHeight || 280;
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
        .attr('stroke', '#2a2d35').attr('stroke-dasharray', '2,3');
    }

    const tooltip = select(tooltipRef.current);

    g.selectAll('rect.bar')
      .data(data)
      .join('rect')
      .attr('class', 'bar')
      .attr('x', (d) => xBand(d.label)!)
      .attr('y', (d) => yScale(d.count))
      .attr('width', xBand.bandwidth())
      .attr('height', (d) => h - yScale(d.count))
      .attr('fill', color)
      .attr('fill-opacity', 0.75)
      .attr('stroke', color)
      .attr('stroke-opacity', 0.4)
      .attr('stroke-width', 0.5)
      .attr('rx', compact ? 1 : 0);

    if (compact) {
      g.selectAll('rect.bar')
        .on('mouseenter', function (_event, d: any) {
          select(this).attr('fill-opacity', 1);
          tooltip
            .style('opacity', '1')
            .html(`<div style="font-size:10px;color:#e0e0e0">${d.label}</div><div style="font-size:11px;font-weight:600;color:#569cd6">${d.count}</div>`);
        })
        .on('mousemove', function (event) {
          const rect = container!.getBoundingClientRect();
          tooltip
            .style('left', `${event.clientX - rect.left + 8}px`)
            .style('top', `${event.clientY - rect.top - 36}px`);
        })
        .on('mouseleave', function () {
          select(this).attr('fill-opacity', 0.75);
          tooltip.style('opacity', '0');
        });
    }

    if (!compact) {
      g.append('g')
        .attr('transform', `translate(0,${h})`)
        .call(axisBottom(xBand).tickSize(-4))
        .call((sel) => {
          sel.select('.domain').attr('stroke', '#3a3d45');
          sel.selectAll('.tick line').attr('stroke', '#3a3d45');
          sel.selectAll('.tick text')
            .attr('fill', '#8b8f9a').attr('font-size', '10px')
            .attr('text-anchor', 'end')
            .attr('transform', data.length > 6 ? 'rotate(-40)' : '');
        });

      g.append('g')
        .call(axisLeft(yScale).ticks(5).tickSize(-4))
        .call((sel) => {
          sel.select('.domain').attr('stroke', '#3a3d45');
          sel.selectAll('.tick line').attr('stroke', '#3a3d45');
          sel.selectAll('.tick text').attr('fill', '#8b8f9a').attr('font-size', '10px');
        });

      if (xLabel) {
        g.append('text')
          .attr('x', w / 2).attr('y', h + 32)
          .attr('text-anchor', 'middle')
          .attr('fill', '#6b7080').attr('font-size', '11px')
          .text(xLabel);
      }
      if (yLabel) {
        g.append('text')
          .attr('transform', 'rotate(-90)')
          .attr('x', -h / 2).attr('y', -42)
          .attr('text-anchor', 'middle')
          .attr('fill', '#6b7080').attr('font-size', '11px')
          .text(yLabel);
      }
    }
  }, [data, xLabel, yLabel, color, heightProp, margin, compact]);

  return (
    <div ref={containerRef} className={`relative rounded-lg border border-gray-800/50 bg-[#13151a] overflow-hidden ${!heightProp ? 'h-full' : ''}`}>
      <svg ref={svgRef} className={!heightProp ? 'w-full h-full' : ''} />
      {compact && (
        <div
          ref={tooltipRef}
          className="absolute pointer-events-none rounded px-2 py-1 bg-[#1e2028] border border-gray-700 shadow-lg opacity-0 transition-opacity duration-100 z-10 whitespace-nowrap"
        />
      )}
    </div>
  );
};

export default Histogram;
