import React, { useEffect, useRef, useState } from 'react';
import { select, scaleLinear, axisBottom, axisLeft, extent, line, timeFormat } from 'd3';

export interface LinePoint {
  x: number;
  y: number;
}

export interface LineProps {
  data: LinePoint[];
  /** X 轴标签 */
  xLabel?: string;
  /** Y 轴标签 */
  yLabel?: string;
  /** X 轴格式: "date"=天数转日期, "datetime"=微秒转日期时间, "number"=普通数值 */
  xFormat?: 'date' | 'datetime' | 'number';
  /** Y 轴格式 */
  yFormat?: 'date' | 'datetime' | 'number';
  /** 线条颜色，默认 #569cd6 */
  color?: string;
  /** 线条宽度，默认 2 */
  strokeWidth?: number;
  /** 是否显示数据点，默认 true */
  showPoints?: boolean;
  /** 图表高度，不传则随容器填充 */
  height?: number;
  /** 图表边距 */
  margin?: { top: number; right: number; bottom: number; left: number };
}

/** 将数值转为 Date（date=天数, datetime=微秒） */
function numToDate(v: number, format: 'date' | 'datetime'): Date {
  if (format === 'date') {
    return new Date(v * 86400000); // days since epoch -> ms
  }
  return new Date(v / 1000); // microseconds -> ms
}

const DEFAULT_MARGIN = { top: 20, right: 24, bottom: 40, left: 56 };
const DEFAULT_COLOR = '#569cd6';

const Line: React.FC<LineProps> = ({
  data,
  xLabel,
  yLabel,
  xFormat = 'number',
  yFormat = 'number',
  color = DEFAULT_COLOR,
  strokeWidth = 2,
  showPoints = true,
  height: heightProp,
  margin = DEFAULT_MARGIN,
}) => {
  const containerRef = useRef<HTMLDivElement>(null);
  const svgRef = useRef<SVGSVGElement>(null);
  const [size, setSize] = useState({ width: 0, height: 0 });

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
    if (!container || data.length === 0 || size.width === 0 || size.height === 0) return;

    const width = size.width;
    const height = heightProp ?? size.height;
    const w = width - margin.left - margin.right;
    const h = height - margin.top - margin.bottom;

    const xExtent = extent(data, (d) => d.x) as [number, number];
    const xPad = (xExtent[1] - xExtent[0]) * 0.06 || 1;
    const xScale = scaleLinear().domain([xExtent[0] - xPad, xExtent[1] + xPad]).range([0, w]);

    const yExtent = extent(data, (d) => d.y) as [number, number];
    const yPad = (yExtent[1] - yExtent[0]) * 0.06 || 1;
    const yScale = scaleLinear().domain([yExtent[0] - yPad, yExtent[1] + yPad]).range([h, 0]);

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
      .attr('stroke', '#2a2d35').attr('stroke-dasharray', '2,3');

    // x axis
    const xAxis = axisBottom(xScale).ticks(6).tickSize(-4);
    if (xFormat === 'date') {
      xAxis.tickFormat((d) => timeFormat('%Y-%m-%d')(numToDate(Number(d), 'date')));
    } else if (xFormat === 'datetime') {
      xAxis.tickFormat((d) => timeFormat('%Y-%m-%d %H:%M')(numToDate(Number(d), 'datetime')));
    }
    g.append('g')
      .attr('transform', `translate(0,${h})`)
      .call(xAxis)
      .call((sel) => {
        sel.select('.domain').attr('stroke', '#3a3d45');
        sel.selectAll('.tick line').attr('stroke', '#3a3d45');
        sel.selectAll('.tick text').attr('fill', '#8b8f9a').attr('font-size', '10px');
      });

    // y axis
    const yAxis = axisLeft(yScale).ticks(5).tickSize(-4);
    if (yFormat === 'date') {
      yAxis.tickFormat((d) => timeFormat('%Y-%m-%d')(numToDate(Number(d), 'date')));
    } else if (yFormat === 'datetime') {
      yAxis.tickFormat((d) => timeFormat('%Y-%m-%d %H:%M')(numToDate(Number(d), 'datetime')));
    }
    g.append('g')
      .call(yAxis)
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

    // line path
    const pathLine = line<LinePoint>()
      .x((d) => xScale(d.x))
      .y((d) => yScale(d.y));

    g.append('path')
      .datum(data)
      .attr('d', pathLine)
      .attr('fill', 'none')
      .attr('stroke', color)
      .attr('stroke-width', strokeWidth)
      .attr('stroke-linecap', 'round')
      .attr('stroke-linejoin', 'round');

    // optional points
    if (showPoints) {
      g.selectAll('circle')
        .data(data)
        .join('circle')
        .attr('cx', (d) => xScale(d.x))
        .attr('cy', (d) => yScale(d.y))
        .attr('r', 3)
        .attr('fill', color)
        .attr('fill-opacity', 0.7)
        .attr('stroke', color)
        .attr('stroke-opacity', 0.3)
        .attr('stroke-width', 1);
    }
  }, [data, xLabel, yLabel, xFormat, yFormat, color, strokeWidth, showPoints, heightProp, margin, size]);

  return (
    <div ref={containerRef} className="w-full h-full min-h-0 rounded-lg border border-gray-800/50 bg-[#13151a] overflow-hidden">
      <svg ref={svgRef} />
    </div>
  );
};

export default Line;
