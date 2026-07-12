/**
 * DID 事件研究图：相对政策时点 rel_time 上 treat×I(rel=k) 的系数与 95% CI（与平行趋势 Wald 同一回归）。
 */
import React, { useEffect, useRef, useState } from 'react';
import { select, scaleLinear, axisBottom, axisLeft } from 'd3';
import { useChartThemeColors } from '@/shared/theme/chartTheme';
import type { DidEventStudyPoint } from '@/shared/types/report';

const MARGIN = { top: 20, right: 16, bottom: 36, left: 52 };

export const DidEventStudyChart: React.FC<{
  points: DidEventStudyPoint[];
  treatLabel?: string;
}> = ({ points, treatLabel = 'Treat' }) => {
  const containerRef = useRef<HTMLDivElement>(null);
  const svgRef = useRef<SVGSVGElement>(null);
  const [size, setSize] = useState({ width: 0, height: 0 });
  const chartTheme = useChartThemeColors();

  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    const ro = new ResizeObserver(() => {
      setSize({ width: el.clientWidth, height: el.clientHeight });
    });
    ro.observe(el);
    setSize({ width: el.clientWidth, height: el.clientHeight });
    return () => ro.disconnect();
  }, []);

  useEffect(() => {
    const svg = select(svgRef.current);
    svg.selectAll('*').remove();

    const sorted = [...points].sort((a, b) => a.rel_time - b.rel_time);
    if (!sorted.length || size.width === 0 || size.height === 0) return;

    const w = size.width - MARGIN.left - MARGIN.right;
    const h = size.height - MARGIN.top - MARGIN.bottom;
    if (w <= 0 || h <= 0) return;

    const xs = sorted.map((d) => d.rel_time);
    const xMin = Math.min(...xs);
    const xMax = Math.max(...xs);
    const xPad = xMax === xMin ? 0.75 : Math.max(0.5, (xMax - xMin) * 0.08);
    const xDomain: [number, number] = [xMin - xPad, xMax + xPad];

    const yCandidates: number[] = [0];
    for (const d of sorted) {
      if (d.is_reference) continue;
      yCandidates.push(d.coef, d.ci_low, d.ci_high);
    }
    let yMin = Math.min(...yCandidates);
    let yMax = Math.max(...yCandidates);
    if (!Number.isFinite(yMin) || !Number.isFinite(yMax)) {
      yMin = -1;
      yMax = 1;
    }
    const yPad = Math.max((yMax - yMin) * 0.12, 0.05 * (Math.max(Math.abs(yMax), Math.abs(yMin)) + 1e-6));
    const yDomain: [number, number] = [yMin - yPad, yMax + yPad];

    const xScale = scaleLinear().domain(xDomain).range([0, w]);
    const yScale = scaleLinear().domain(yDomain).range([h, 0]);

    const g = svg
      .attr('width', size.width)
      .attr('height', size.height)
      .append('g')
      .attr('transform', `translate(${MARGIN.left},${MARGIN.top})`);

    g.append('rect').attr('width', w).attr('height', h).attr('fill', chartTheme.canvas).attr('rx', 2);

    g.append('line')
      .attr('x1', 0)
      .attr('x2', w)
      .attr('y1', yScale(0))
      .attr('y2', yScale(0))
      .attr('stroke', chartTheme.zeroLine)
      .attr('stroke-dasharray', '4,3');

    if (0 >= xDomain[0] && 0 <= xDomain[1]) {
      const x0 = xScale(0);
      g.append('line')
        .attr('x1', x0)
        .attr('x2', x0)
        .attr('y1', 0)
        .attr('y2', h)
        .attr('stroke', chartTheme.label)
        .attr('stroke-dasharray', '3,3')
        .attr('opacity', 0.6);
    }

    const xAxis = axisBottom(xScale).ticks(Math.min(12, sorted.length + 2)).tickFormat((d) => String(d));
    const yAxis = axisLeft(yScale).ticks(6);

    g.append('g')
      .attr('transform', `translate(0,${h})`)
      .call(xAxis)
      .attr('color', chartTheme.tick)
      .selectAll('path,line')
      .attr('stroke', chartTheme.axis);

    g.append('g')
      .call(yAxis)
      .attr('color', chartTheme.tick)
      .selectAll('path,line')
      .attr('stroke', chartTheme.axis);

    g.append('text')
      .attr('x', w / 2)
      .attr('y', h + 28)
      .attr('text-anchor', 'middle')
      .attr('fill', chartTheme.label)
      .attr('font-size', 11)
      .text('Relative time (rel_time)');

    g.append('text')
      .attr('transform', 'rotate(-90)')
      .attr('x', -h / 2)
      .attr('y', -40)
      .attr('text-anchor', 'middle')
      .attr('fill', chartTheme.label)
      .attr('font-size', 11)
      .text(`Coefficient (× ${treatLabel})`);

    const dotR = 5;
    for (const d of sorted) {
      const x = xScale(d.rel_time);
      const y = yScale(d.coef);
      if (!d.is_reference && d.std_err > 0 && d.ci_high >= d.ci_low) {
        const y1 = yScale(d.ci_high);
        const y2 = yScale(d.ci_low);
        g.append('line')
          .attr('x1', x)
          .attr('x2', x)
          .attr('y1', y1)
          .attr('y2', y2)
          .attr('stroke', '#22d3ee')
          .attr('stroke-width', 1.5)
          .attr('stroke-linecap', 'round');
        const cap = 4;
        g.append('line')
          .attr('x1', x - cap)
          .attr('x2', x + cap)
          .attr('y1', y1)
          .attr('y2', y1)
          .attr('stroke', '#22d3ee')
          .attr('stroke-width', 1.5);
        g.append('line')
          .attr('x1', x - cap)
          .attr('x2', x + cap)
          .attr('y1', y2)
          .attr('y2', y2)
          .attr('stroke', '#22d3ee')
          .attr('stroke-width', 1.5);
      }
      g.append('circle')
        .attr('cx', x)
        .attr('cy', y)
        .attr('r', d.is_reference ? dotR - 0.5 : dotR)
        .attr('fill', d.is_reference ? chartTheme.tick : '#22d3ee')
        .attr('stroke', d.is_reference ? '#d1d5db' : '#a5f3fc')
        .attr('stroke-width', 1.2);
    }

    const linePts = sorted.filter((d) => !d.is_reference);
    if (linePts.length >= 2) {
      const pathD = linePts
        .map((d, i) => `${i === 0 ? 'M' : 'L'} ${xScale(d.rel_time)} ${yScale(d.coef)}`)
        .join(' ');
      g.append('path')
        .attr('d', pathD)
        .attr('fill', 'none')
        .attr('stroke', '#22d3ee')
        .attr('stroke-width', 1)
        .attr('opacity', 0.35)
        .attr('stroke-linejoin', 'round');
    }
  }, [points, size, treatLabel, chartTheme]);

  if (!points.length) return null;

  return (
    <div ref={containerRef} className="w-full h-[min(320px,50vh)] min-h-[220px] mt-3">
      <svg ref={svgRef} className="block w-full h-full" role="img" aria-label="Event study coefficients" />
    </div>
  );
};
