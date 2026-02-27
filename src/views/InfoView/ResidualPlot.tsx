import React, { useEffect, useRef } from 'react';
import { select, scaleLinear, axisBottom, axisLeft, extent } from 'd3';

const CHART_MARGIN = { top: 20, right: 24, bottom: 40, left: 56 };
const CHART_HEIGHT = 280;

const ResidualPlot: React.FC<{ fitted: number[]; residuals: number[] }> = ({ fitted, residuals }) => {
  const containerRef = useRef<HTMLDivElement>(null);
  const svgRef = useRef<SVGSVGElement>(null);

  useEffect(() => {
    const svg = select(svgRef.current);
    svg.selectAll('*').remove();

    const container = containerRef.current;
    if (!container || fitted.length === 0) return;

    const width = container.clientWidth;
    const m = CHART_MARGIN;
    const w = width - m.left - m.right;
    const h = CHART_HEIGHT - m.top - m.bottom;

    const data = fitted.map((f, i) => ({ fitted: f, residual: residuals[i] }));

    const xExtent = extent(data, (d) => d.fitted) as [number, number];
    const yExtent = extent(data, (d) => d.residual) as [number, number];
    const yMax = Math.max(Math.abs(yExtent[0]), Math.abs(yExtent[1])) * 1.15;
    const xPad = (xExtent[1] - xExtent[0]) * 0.06 || 1;

    const x = scaleLinear().domain([xExtent[0] - xPad, xExtent[1] + xPad]).range([0, w]);
    const y = scaleLinear().domain([-yMax, yMax]).range([h, 0]);

    const g = svg
      .attr('width', width)
      .attr('height', CHART_HEIGHT)
      .append('g')
      .attr('transform', `translate(${m.left},${m.top})`);

    g.append('g')
      .attr('class', 'grid-y')
      .selectAll('line')
      .data(y.ticks(5))
      .join('line')
      .attr('x1', 0).attr('x2', w)
      .attr('y1', (d) => y(d)).attr('y2', (d) => y(d))
      .attr('stroke', '#2a2d35').attr('stroke-dasharray', '2,3');

    g.append('line')
      .attr('x1', 0).attr('x2', w)
      .attr('y1', y(0)).attr('y2', y(0))
      .attr('stroke', '#4a4d55').attr('stroke-width', 1);

    g.append('g')
      .attr('transform', `translate(0,${h})`)
      .call(axisBottom(x).ticks(6).tickSize(-4))
      .call((sel) => {
        sel.select('.domain').attr('stroke', '#3a3d45');
        sel.selectAll('.tick line').attr('stroke', '#3a3d45');
        sel.selectAll('.tick text').attr('fill', '#8b8f9a').attr('font-size', '10px');
      });

    g.append('g')
      .call(axisLeft(y).ticks(5).tickSize(-4))
      .call((sel) => {
        sel.select('.domain').attr('stroke', '#3a3d45');
        sel.selectAll('.tick line').attr('stroke', '#3a3d45');
        sel.selectAll('.tick text').attr('fill', '#8b8f9a').attr('font-size', '10px');
      });

    g.append('text')
      .attr('x', w / 2).attr('y', h + 32)
      .attr('text-anchor', 'middle')
      .attr('fill', '#6b7080').attr('font-size', '11px')
      .text('Fitted Values');

    g.append('text')
      .attr('transform', 'rotate(-90)')
      .attr('x', -h / 2).attr('y', -42)
      .attr('text-anchor', 'middle')
      .attr('fill', '#6b7080').attr('font-size', '11px')
      .text('Residuals');

    g.selectAll('circle')
      .data(data)
      .join('circle')
      .attr('cx', (d) => x(d.fitted))
      .attr('cy', (d) => y(d.residual))
      .attr('r', 3)
      .attr('fill', '#569cd6')
      .attr('fill-opacity', 0.7)
      .attr('stroke', '#569cd6')
      .attr('stroke-opacity', 0.3)
      .attr('stroke-width', 1);
  }, [fitted, residuals]);

  return (
    <div ref={containerRef} className="rounded-lg border border-gray-800/50 bg-[#13151a] overflow-hidden">
      <svg ref={svgRef} />
    </div>
  );
};

export default ResidualPlot;
