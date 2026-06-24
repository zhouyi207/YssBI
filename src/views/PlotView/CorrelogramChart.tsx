/**
 * Correlogram (ACF/PACF) 图
 *
 * Stata 风格：垂直柱状图，y 轴 -1..1，置信区间 ±1.96/√n
 * Hover tooltip 显示 lag、value、Q 统计量及 p-value
 */
import React, { useEffect, useRef, useState, useCallback } from 'react';
import { select, scaleLinear, scaleBand, axisBottom, axisLeft } from 'd3';
import { useChartThemeColors } from '@/shared/theme/chartTheme';

export interface CorrelogramDatum {
  lag: number;
  value: number;
  q_stat: number;
  p_value: number;
}

export interface CorrelogramChartProps {
  data: CorrelogramDatum[];
  ciHalfWidth: number;
  title?: string;
  color?: string;
  /** "acf" or "pacf" – used for tooltip label */
  valueLabel?: string;
}

const MARGIN = { top: 28, right: 24, bottom: 36, left: 52 };
const DEFAULT_COLOR = '#569cd6';

const CorrelogramChart: React.FC<CorrelogramChartProps> = ({
  data,
  ciHalfWidth,
  title,
  color = DEFAULT_COLOR,
  valueLabel = 'Value',
}) => {
  const containerRef = useRef<HTMLDivElement>(null);
  const svgRef = useRef<SVGSVGElement>(null);
  const tooltipRef = useRef<HTMLDivElement>(null);
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

    const g = svg
      .attr('width', size.width)
      .attr('height', size.height)
      .append('g')
      .attr('transform', `translate(${MARGIN.left},${MARGIN.top})`);

    const xBand = scaleBand()
      .domain(data.map((d) => String(d.lag)))
      .range([0, w])
      .padding(0.25);

    const yExtent = Math.max(1, Math.abs(ciHalfWidth) * 1.2);
    const yScale = scaleLinear()
      .domain([-yExtent, yExtent])
      .range([h, 0]);

    const zeroY = yScale(0);

    g.append('rect')
      .attr('x', 0)
      .attr('y', yScale(ciHalfWidth))
      .attr('width', w)
      .attr('height', yScale(-ciHalfWidth) - yScale(ciHalfWidth))
      .attr('fill', chartTheme.grid)
      .attr('opacity', 0.5);

    [ciHalfWidth, -ciHalfWidth].forEach((ci) => {
      g.append('line')
        .attr('x1', 0).attr('x2', w)
        .attr('y1', yScale(ci)).attr('y2', yScale(ci))
        .attr('stroke', chartTheme.zeroLine)
        .attr('stroke-dasharray', '4,4');
    });

    g.append('line')
      .attr('x1', 0).attr('x2', w)
      .attr('y1', zeroY).attr('y2', zeroY)
      .attr('stroke', chartTheme.zeroLine);

    const tipEl = tooltipRef.current;

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
        .attr('rx', 2)
        .style('cursor', 'pointer')
        .on('mouseenter', function (event) {
          select(this).attr('fill-opacity', 1).attr('stroke', '#fff').attr('stroke-width', 1);
          if (!tipEl) return;
          tipEl.style.opacity = '1';
          tipEl.innerHTML =
            `<div style="font-size:11px;line-height:1.6;color:${chartTheme.tooltipFg}">` +
            `<b>Lag ${d.lag}</b><br/>` +
            `${valueLabel}: <b>${d.value.toFixed(4)}</b><br/>` +
            `Q(${d.lag}): <b>${d.q_stat.toFixed(4)}</b><br/>` +
            `p-value: <b>${d.p_value < 0.0001 ? d.p_value.toExponential(2) : d.p_value.toFixed(4)}</b>` +
            `</div>`;
          const rect = (event.currentTarget as SVGRectElement).getBoundingClientRect();
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
          select(this).attr('fill-opacity', 0.85).attr('stroke', 'none');
          hideTooltip();
        });
    });

    g.append('g')
      .attr('transform', `translate(0,${h})`)
      .call(axisBottom(xBand).tickSize(-4))
      .call((sel) => {
        sel.select('.domain').attr('stroke', chartTheme.axis);
        sel.selectAll('.tick line').attr('stroke', chartTheme.axis);
        sel.selectAll('.tick text').attr('fill', chartTheme.tick).attr('font-size', '10px');
      });

    g.append('g')
      .call(axisLeft(yScale).ticks(5).tickSize(-4))
      .call((sel) => {
        sel.select('.domain').attr('stroke', chartTheme.axis);
        sel.selectAll('.tick line').attr('stroke', chartTheme.axis);
        sel.selectAll('.tick text').attr('fill', chartTheme.tick).attr('font-size', '10px');
      });

    if (title) {
      g.append('text')
        .attr('x', w / 2)
        .attr('y', -10)
        .attr('text-anchor', 'middle')
        .attr('fill', chartTheme.tick)
        .attr('font-size', '12px')
        .attr('font-weight', '500')
        .text(title);
    }
  }, [data, ciHalfWidth, title, color, valueLabel, size, hideTooltip, chartTheme]);

  return (
    <div ref={containerRef} className="relative w-full flex-1 min-h-0 rounded-lg border border-gray-800/50 bg-[#13151a] overflow-hidden">
      <svg ref={svgRef} style={{ width: '100%', height: '100%' }} />
      <div
        ref={tooltipRef}
        className="pointer-events-none absolute z-10 rounded-md bg-[#1e2028] border border-gray-700/60 px-3 py-2 shadow-lg transition-opacity duration-100"
        style={{ opacity: 0 }}
      />
    </div>
  );
};

export default CorrelogramChart;
