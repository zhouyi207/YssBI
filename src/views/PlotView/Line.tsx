import React, { useEffect, useRef, useState, useCallback } from 'react';
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
  /** 是否显示数据点（初始值），默认 true */
  showPoints?: boolean;
  /** 图表高度，不传则随容器填充 */
  height?: number;
  /** 图表边距 */
  margin?: { top: number; right: number; bottom: number; left: number };
}

function ToolbarToggle({ checked, onChange, label }: { checked: boolean; onChange: (v: boolean) => void; label: string }) {
  return (
    <label className="flex items-center gap-1.5 cursor-pointer select-none">
      <button
        role="switch"
        aria-checked={checked}
        onClick={() => onChange(!checked)}
        className={`relative w-7 h-4 rounded-full transition-colors duration-200 ${checked ? 'bg-[#569cd6]' : 'bg-gray-600'}`}
      >
        <span
          className={`absolute top-0.5 left-0.5 w-3 h-3 rounded-full bg-white transition-transform duration-200 ${checked ? 'translate-x-3' : 'translate-x-0'}`}
        />
      </button>
      <span className="text-[11px] text-gray-400">{label}</span>
    </label>
  );
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
  showPoints: showPointsInit = true,
  height: heightProp,
  margin = DEFAULT_MARGIN,
}) => {
  const containerRef = useRef<HTMLDivElement>(null);
  const svgRef = useRef<SVGSVGElement>(null);
  const [size, setSize] = useState({ width: 0, height: 0 });
  const [toolbarOpen, setToolbarOpen] = useState(false);
  const [pointsVisible, setPointsVisible] = useState(showPointsInit);

  const toggleToolbar = useCallback(() => setToolbarOpen((v) => !v), []);

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
    if (pointsVisible) {
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
  }, [data, xLabel, yLabel, xFormat, yFormat, color, strokeWidth, pointsVisible, heightProp, margin, size]);

  return (
    <div className="w-full h-full min-h-0 flex flex-col rounded-lg border border-gray-800/50 bg-[#13151a] overflow-hidden">
      {/* toolbar toggle */}
      <div className="flex items-center justify-end px-2 pt-1.5 pb-0">
        <button
          onClick={toggleToolbar}
          title="Toggle toolbar"
          className={`p-1 rounded transition-colors ${toolbarOpen ? 'text-[#569cd6] bg-[#569cd6]/10' : 'text-gray-500 hover:text-gray-300'}`}
        >
          <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" strokeWidth={2} viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.066 2.573c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.573 1.066c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.066-2.573c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z" />
            <circle cx="12" cy="12" r="3" />
          </svg>
        </button>
      </div>

      {/* toolbar */}
      {toolbarOpen && (
        <div className="flex items-center gap-4 px-3 py-1.5 border-b border-gray-800/50">
          <ToolbarToggle checked={pointsVisible} onChange={setPointsVisible} label="Scatter Points" />
        </div>
      )}

      {/* chart */}
      <div ref={containerRef} className="flex-1 min-h-0">
        <svg ref={svgRef} />
      </div>
    </div>
  );
};

export default Line;
