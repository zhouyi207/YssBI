import React, { useEffect, useRef, useState } from 'react';
import { select, scaleLinear, axisBottom, axisLeft, extent } from 'd3';

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
  /** 散点颜色，默认 #569cd6 */
  color?: string;
  /** 散点半径，默认 3 */
  radius?: number;
  /** 图表高度，不传则随容器填充 */
  height?: number;
  /** 图表边距 */
  margin?: { top: number; right: number; bottom: number; left: number };
  /** Y 轴是否关于 0 对称（如残差图），默认 false */
  symmetricY?: boolean;
  /** 是否绘制 y=0 参考线，默认 false */
  zeroLine?: boolean;
}

const DEFAULT_MARGIN = { top: 20, right: 24, bottom: 40, left: 56 };
const DEFAULT_COLOR = '#569cd6';

const Scatter: React.FC<ScatterProps> = ({
  data,
  xLabel,
  yLabel,
  color = DEFAULT_COLOR,
  radius = 3,
  height: heightProp,
  margin = DEFAULT_MARGIN,
  symmetricY = false,
  zeroLine = false,
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
      .attr('stroke', '#2a2d35').attr('stroke-dasharray', '2,3');

    // zero reference line
    if (zeroLine) {
      g.append('line')
        .attr('x1', 0).attr('x2', w)
        .attr('y1', yScale(0)).attr('y2', yScale(0))
        .attr('stroke', '#4a4d55').attr('stroke-width', 1);
    }

    // x axis
    g.append('g')
      .attr('transform', `translate(0,${h})`)
      .call(axisBottom(xScale).ticks(6).tickSize(-4))
      .call((sel) => {
        sel.select('.domain').attr('stroke', '#3a3d45');
        sel.selectAll('.tick line').attr('stroke', '#3a3d45');
        sel.selectAll('.tick text').attr('fill', '#8b8f9a').attr('font-size', '10px');
      });

    // y axis
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

    // points
    g.selectAll('circle')
      .data(data)
      .join('circle')
      .attr('cx', (d) => xScale(d.x))
      .attr('cy', (d) => yScale(d.y))
      .attr('r', radius)
      .attr('fill', color)
      .attr('fill-opacity', 0.7)
      .attr('stroke', color)
      .attr('stroke-opacity', 0.3)
      .attr('stroke-width', 1);
  }, [data, xLabel, yLabel, color, radius, heightProp, margin, symmetricY, zeroLine, size]);

  return (
    <div ref={containerRef} className="w-full h-full min-h-0 rounded-lg border border-gray-800/50 bg-[#13151a] overflow-hidden">
      <svg ref={svgRef} />
    </div>
  );
};

export default Scatter;
