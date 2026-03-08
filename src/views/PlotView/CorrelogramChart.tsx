/**
 * Correlogram (ACF/PACF) 图
 *
 * Stata 风格：垂直柱状图，y 轴 -1..1，置信区间 ±1.96/√n
 */
import React, { useEffect, useRef, useState } from 'react';
import { select, scaleLinear, scaleBand, axisBottom, axisLeft } from 'd3';

export interface CorrelogramDatum {
  lag: number;
  value: number;
}

export interface CorrelogramChartProps {
  data: CorrelogramDatum[];
  /** 置信区间半宽（如 1.96/√n） */
  ciHalfWidth: number;
  title?: string;
  height?: number;
  color?: string;
}

const DEFAULT_MARGIN = { top: 24, right: 24, bottom: 40, left: 48 };
const DEFAULT_COLOR = '#569cd6';

const CorrelogramChart: React.FC<CorrelogramChartProps> = ({
  data,
  ciHalfWidth,
  title,
  height = 240,
  color = DEFAULT_COLOR,
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

    if (data.length === 0 || size.width === 0) return;

    const margin = DEFAULT_MARGIN;
    const width = size.width;
    const w = width - margin.left - margin.right;
    const h = height - margin.top - margin.bottom;
    if (w <= 0 || h <= 0) return;

    const g = svg
      .attr('width', width)
      .attr('height', height)
      .append('g')
      .attr('transform', `translate(${margin.left},${margin.top})`);

    const xBand = scaleBand()
      .domain(data.map((d) => String(d.lag)))
      .range([0, w])
      .padding(0.25);

    const yExtent = Math.max(1, Math.abs(ciHalfWidth) * 1.2);
    const yScale = scaleLinear()
      .domain([-yExtent, yExtent])
      .range([h, 0]);

    const zeroY = yScale(0);

    // 置信区间带（灰色）
    g.append('rect')
      .attr('x', 0)
      .attr('y', yScale(ciHalfWidth))
      .attr('width', w)
      .attr('height', yScale(-ciHalfWidth) - yScale(ciHalfWidth))
      .attr('fill', '#2a2d35')
      .attr('opacity', 0.5);

    // 置信区间线 ±1.96/√n
    g.append('line')
      .attr('x1', 0)
      .attr('x2', w)
      .attr('y1', yScale(ciHalfWidth))
      .attr('y2', yScale(ciHalfWidth))
      .attr('stroke', '#4a4d55')
      .attr('stroke-dasharray', '4,4');

    g.append('line')
      .attr('x1', 0)
      .attr('x2', w)
      .attr('y1', yScale(-ciHalfWidth))
      .attr('y2', yScale(-ciHalfWidth))
      .attr('stroke', '#4a4d55')
      .attr('stroke-dasharray', '4,4');

    // y=0 线
    g.append('line')
      .attr('x1', 0)
      .attr('x2', w)
      .attr('y1', zeroY)
      .attr('y2', zeroY)
      .attr('stroke', '#5a5d65');

    // 柱状图：从 0 到 value
    data.forEach((d) => {
      const x = xBand(String(d.lag))!;
      const bw = xBand.bandwidth();
      const y0 = zeroY;
      const y1 = yScale(d.value);
      const yMin = Math.min(y0, y1);
      const yMax = Math.max(y0, y1);
      const barH = yMax - yMin || 1;

      g.append('rect')
        .attr('x', x)
        .attr('y', yMin)
        .attr('width', bw)
        .attr('height', barH)
        .attr('fill', d.value >= 0 ? color : '#e06c75')
        .attr('fill-opacity', 0.85)
        .attr('rx', 2);
    });

    g.append('g')
      .attr('transform', `translate(0,${h})`)
      .call(axisBottom(xBand).tickSize(-4))
      .call((sel) => {
        sel.select('.domain').attr('stroke', '#3a3d45');
        sel.selectAll('.tick line').attr('stroke', '#3a3d45');
        sel.selectAll('.tick text').attr('fill', '#8b8f9a').attr('font-size', '10px');
      });

    g.append('g')
      .call(axisLeft(yScale).ticks(5).tickSize(-4))
      .call((sel) => {
        sel.select('.domain').attr('stroke', '#3a3d45');
        sel.selectAll('.tick line').attr('stroke', '#3a3d45');
        sel.selectAll('.tick text').attr('fill', '#8b8f9a').attr('font-size', '10px');
      });

    if (title) {
      g.append('text')
        .attr('x', w / 2)
        .attr('y', -8)
        .attr('text-anchor', 'middle')
        .attr('fill', '#8b8f9a')
        .attr('font-size', '11px')
        .text(title);
    }
  }, [data, ciHalfWidth, title, height, color, size]);

  return (
    <div ref={containerRef} className="w-full rounded-lg border border-gray-800/50 bg-[#13151a] overflow-hidden">
      <svg ref={svgRef} style={{ width: '100%', height }} />
    </div>
  );
};

export default CorrelogramChart;
