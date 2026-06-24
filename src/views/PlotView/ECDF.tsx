import React, { useEffect, useRef, useState } from 'react';
import { select, scaleLinear, axisBottom, axisLeft, extent, line, curveStepAfter } from 'd3';
import { useChartThemeColors } from '@/shared/theme/chartTheme';

export interface ECDFPoint {
  x: number;
  y: number;
}

export interface ECDFProps {
  /** ECDF 数据点：(x, F(x))，x 为排序后的值，y 为累积比例 [0, 1] */
  data: ECDFPoint[];
  /** X 轴标签 */
  xLabel?: string;
  /** Y 轴标签，默认 "Cumulative Proportion" */
  yLabel?: string;
  /** 线条颜色，默认 #569cd6 */
  color?: string;
  /** 图表高度，不传则随容器填充 */
  height?: number;
  /** 图表边距 */
  margin?: { top: number; right: number; bottom: number; left: number };
}

const DEFAULT_MARGIN = { top: 20, right: 24, bottom: 40, left: 56 };
const DEFAULT_COLOR = '#569cd6';

const ECDF: React.FC<ECDFProps> = ({
  data,
  xLabel,
  yLabel = 'Cumulative Proportion',
  color = DEFAULT_COLOR,
  height: heightProp,
  margin = DEFAULT_MARGIN,
}) => {
  const containerRef = useRef<HTMLDivElement>(null);
  const svgRef = useRef<SVGSVGElement>(null);
  const [size, setSize] = useState({ width: 0, height: 0 });
  const chartTheme = useChartThemeColors();

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
    const xScale = scaleLinear()
      .domain([xExtent[0] - xPad, xExtent[1] + xPad])
      .range([0, w]);

    const yScale = scaleLinear().domain([0, 1]).nice().range([h, 0]);

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
      .attr('x1', 0)
      .attr('x2', w)
      .attr('y1', (d) => yScale(d))
      .attr('y2', (d) => yScale(d))
      .attr('stroke', chartTheme.grid)
      .attr('stroke-dasharray', '2,3');

    // x axis
    g.append('g')
      .attr('transform', `translate(0,${h})`)
      .call(axisBottom(xScale).ticks(6).tickSize(-4))
      .call((sel) => {
        sel.select('.domain').attr('stroke', chartTheme.axis);
        sel.selectAll('.tick line').attr('stroke', chartTheme.axis);
        sel.selectAll('.tick text').attr('fill', chartTheme.tick).attr('font-size', '10px');
      });

    // y axis
    g.append('g')
      .call(axisLeft(yScale).ticks(5).tickSize(-4))
      .call((sel) => {
        sel.select('.domain').attr('stroke', chartTheme.axis);
        sel.selectAll('.tick line').attr('stroke', chartTheme.axis);
        sel.selectAll('.tick text').attr('fill', chartTheme.tick).attr('font-size', '10px');
      });

    if (xLabel) {
      g.append('text')
        .attr('x', w / 2)
        .attr('y', h + 32)
        .attr('text-anchor', 'middle')
        .attr('fill', chartTheme.label)
        .attr('font-size', '11px')
        .text(xLabel);
    }

    if (yLabel) {
      g.append('text')
        .attr('transform', 'rotate(-90)')
        .attr('x', -h / 2)
        .attr('y', -42)
        .attr('text-anchor', 'middle')
        .attr('fill', chartTheme.label)
        .attr('font-size', '11px')
        .text(yLabel);
    }

    // ECDF 阶梯线：左连续，从 (x_min, 0) 开始
    const sorted = [...data].sort((a, b) => a.x - b.x);
    const stepPoints: { x: number; y: number }[] = [];
    if (sorted.length > 0) {
      const xMin = xExtent[0] - xPad;
      stepPoints.push({ x: xMin, y: 0 });
      for (const p of sorted) {
        stepPoints.push({ x: p.x, y: p.y });
      }
    }

    if (stepPoints.length > 1) {
      const pathLine = line<{ x: number; y: number }>()
        .x((d) => xScale(d.x))
        .y((d) => yScale(d.y))
        .curve(curveStepAfter);

      g.append('path')
        .datum(stepPoints)
        .attr('d', pathLine)
        .attr('fill', 'none')
        .attr('stroke', color)
        .attr('stroke-width', 2)
        .attr('stroke-linecap', 'round')
        .attr('stroke-linejoin', 'round');
    }
  }, [data, xLabel, yLabel, color, heightProp, margin, size, chartTheme]);

  return (
    <div ref={containerRef} className="w-full h-full min-h-0 rounded-lg border border-gray-800/50 bg-[#13151a] overflow-hidden">
      <svg ref={svgRef} />
    </div>
  );
};

export default ECDF;
