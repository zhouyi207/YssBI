/**
 * VAR 特征值平稳性图（Stata varstable, graph）
 *
 * 在复平面上绘制特征值，单位圆（模=1）作为参考。
 * X 轴：实部，Y 轴：虚部。
 */
import React, { useEffect, useRef, useState, useCallback } from 'react';
import { select, scaleLinear, axisBottom, axisLeft } from 'd3';
import { useChartThemeColors, useChartSeriesColors } from '@/shared/theme/chartTheme';
import type { VARStableRow } from './types';

export interface VARStableChartProps {
  data: VARStableRow[];
}

const MARGIN = { top: 28, right: 24, bottom: 40, left: 48 };
const DEFAULT_RANGE = 1.3; // 显示范围 [-1.3, 1.3] 以包含单位圆

const VARStableChart: React.FC<VARStableChartProps> = ({ data }) => {
  const containerRef = useRef<HTMLDivElement>(null);
  const svgRef = useRef<SVGSVGElement>(null);
  const tooltipRef = useRef<HTMLDivElement>(null);
  const [size, setSize] = useState({ width: 0, height: 0 });
  const chartTheme = useChartThemeColors();
  const seriesColors = useChartSeriesColors();

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

  const hideTooltip = useCallback(() => {
    const tip = tooltipRef.current;
    if (tip) tip.style.opacity = '0';
  }, []);

  useEffect(() => {
    const svg = select(svgRef.current);
    svg.selectAll('*').remove();

    if (data.length === 0 || size.width === 0 || size.height === 0) return;

    const w = size.width - MARGIN.left - MARGIN.right;
    const h = size.height - MARGIN.top - MARGIN.bottom;
    if (w <= 0 || h <= 0) return;

    const plotSize = Math.min(w, h);
    const k = plotSize / (2 * DEFAULT_RANGE);

    const xScale = scaleLinear()
      .domain([-DEFAULT_RANGE, DEFAULT_RANGE])
      .range([w / 2 - plotSize / 2, w / 2 + plotSize / 2]);

    const yScale = scaleLinear()
      .domain([DEFAULT_RANGE, -DEFAULT_RANGE])
      .range([h / 2 - plotSize / 2, h / 2 + plotSize / 2]);

    const g = svg
      .attr('width', size.width)
      .attr('height', size.height)
      .append('g')
      .attr('transform', `translate(${MARGIN.left},${MARGIN.top})`);

    // 极坐标网格圆 (0.2, 0.4, 0.6, 0.8, 1.0)
    [0.2, 0.4, 0.6, 0.8, 1.0].forEach((radius) => {
      const rPx = radius * k;
      g.append('circle')
        .attr('cx', w / 2)
        .attr('cy', h / 2)
        .attr('r', rPx)
        .attr('fill', 'none')
        .attr('stroke', radius === 1 ? chartTheme.zeroLine : chartTheme.grid)
        .attr('stroke-width', radius === 1 ? 1.5 : 0.8)
        .attr('stroke-dasharray', radius === 1 ? 'none' : '3,3');
    });

    // 实轴、虚轴
    g.append('line')
      .attr('x1', 0).attr('x2', w)
      .attr('y1', h / 2).attr('y2', h / 2)
      .attr('stroke', chartTheme.axis).attr('stroke-width', 1);
    g.append('line')
      .attr('x1', w / 2).attr('x2', w / 2)
      .attr('y1', 0).attr('y2', h)
      .attr('stroke', chartTheme.axis).attr('stroke-width', 1);

    const tipEl = tooltipRef.current;

    // 特征值点
    data.forEach((d, i) => {
      const x = xScale(d.re);
      const y = yScale(d.im);
      const isUnstable = d.modulus >= 1.0;

      g.append('circle')
        .attr('cx', x)
        .attr('cy', y)
        .attr('r', 5)
        .attr('fill', isUnstable ? seriesColors.negative : seriesColors.primary)
        .attr('stroke', isUnstable ? seriesColors.negative : seriesColors.primary)
        .attr('stroke-width', 1.5)
        .attr('fill-opacity', 0.9)
        .style('cursor', 'pointer')
        .on('mouseenter', function (event) {
          select(this).attr('r', 6).attr('stroke-width', 2);
          if (!tipEl) return;
          const evStr = d.im >= 0
            ? `${d.re.toFixed(4)} + ${d.im.toFixed(4)}i`
            : `${d.re.toFixed(4)} - ${Math.abs(d.im).toFixed(4)}i`;
          tipEl.style.opacity = '1';
          tipEl.innerHTML =
            `<div style="font-size:11px;line-height:1.6">` +
            `<b>Eigenvalue ${i + 1}</b><br/>` +
            `${evStr}<br/>` +
            `Modulus: <b>${d.modulus.toFixed(6)}</b>` +
            (isUnstable ? `<br/><span style="color:${seriesColors.negative}">≥ 1 (unstable)</span>` : '') +
            `</div>`;
          const rect = (event.currentTarget as SVGCircleElement).getBoundingClientRect();
          const containerRect = containerRef.current!.getBoundingClientRect();
          const tipW = tipEl.offsetWidth;
          let left = rect.left + rect.width / 2 - containerRect.left - tipW / 2;
          left = Math.max(4, Math.min(left, containerRect.width - tipW - 4));
          const above = rect.top - containerRect.top - tipEl.offsetHeight - 6;
          const below = rect.bottom - containerRect.top + 6;
          tipEl.style.left = `${left}px`;
          tipEl.style.top = above > 0 ? `${above}px` : `${below}px`;
        })
        .on('mouseleave', function () {
          select(this).attr('r', 5).attr('stroke-width', 1.5);
          hideTooltip();
        });
    });

    g.append('g')
      .attr('transform', `translate(0,${h})`)
      .call(axisBottom(xScale).ticks(6).tickSize(-4))
      .call((sel) => {
        sel.select('.domain').attr('stroke', chartTheme.axis);
        sel.selectAll('.tick line').attr('stroke', chartTheme.axis);
        sel.selectAll('.tick text').attr('fill', chartTheme.tick).attr('font-size', '10px');
      });

    g.append('g')
      .call(axisLeft(yScale).ticks(6).tickSize(-4))
      .call((sel) => {
        sel.select('.domain').attr('stroke', chartTheme.axis);
        sel.selectAll('.tick line').attr('stroke', chartTheme.axis);
        sel.selectAll('.tick text').attr('fill', chartTheme.tick).attr('font-size', '10px');
      });

    g.append('text')
      .attr('x', w / 2)
      .attr('y', -10)
      .attr('text-anchor', 'middle')
      .attr('fill', chartTheme.label)
      .attr('font-size', '11px')
      .text('Real');
    g.append('text')
      .attr('transform', 'rotate(-90)')
      .attr('x', -h / 2)
      .attr('y', -36)
      .attr('text-anchor', 'middle')
      .attr('fill', chartTheme.label)
      .attr('font-size', '11px')
      .text('Imaginary');
  }, [data, size, hideTooltip, chartTheme, seriesColors]);

  return (
    <div ref={containerRef} className="relative w-full flex-1 min-h-0 rounded-lg border border-border overflow-hidden" style={{ backgroundColor: chartTheme.canvas }}>
      <svg ref={svgRef} style={{ width: '100%', height: '100%' }} />
      <div
        ref={tooltipRef}
        className="pointer-events-none absolute z-10 rounded-md bg-popover border border-border px-3 py-2 shadow-lg transition-opacity duration-100"
        style={{ opacity: 0 }}
      />
    </div>
  );
};

export default VARStableChart;
