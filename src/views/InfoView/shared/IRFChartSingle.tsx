/**
 * 单张脉冲响应图（一个 impulse→response 的 IRF 折线图）
 */
import React, { useEffect, useRef, useState, useCallback } from 'react';
import { select, scaleLinear, axisBottom, axisLeft, line } from 'd3';
import { useChartThemeColors, useChartSeriesColors } from '@/shared/theme/chartTheme';

export interface IRFChartSingleProps {
  /** 序列：step -> value */
  series: { step: number; value: number }[];
  /** 95% CI 下界（可选） */
  lower?: number[];
  /** 95% CI 上界（可选） */
  upper?: number[];
  /** 子图标题，如 "y→lny" */
  title: string;
}

const MARGIN = { top: 24, right: 12, bottom: 24, left: 36 };

const IRFChartSingle: React.FC<IRFChartSingleProps> = ({ series, lower, upper, title }) => {
  const containerRef = useRef<HTMLDivElement>(null);
  const svgRef = useRef<SVGSVGElement>(null);
  const tooltipRef = useRef<HTMLDivElement>(null);
  const [size, setSize] = useState({ width: 0, height: 0 });
  const chartTheme = useChartThemeColors();
  const seriesColors = useChartSeriesColors();
  const plotColor = seriesColors.primary;

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

    if (!series?.length || size.width === 0 || size.height === 0) return;

    const w = size.width - MARGIN.left - MARGIN.right;
    const h = size.height - MARGIN.top - MARGIN.bottom;
    if (w <= 0 || h <= 0) return;

    const nSteps = series.length;
    const values = series.map((d) => d.value);
    const allVals = lower && upper
      ? [...values, ...lower, ...upper]
      : values;
    const yMin = Math.min(0, ...allVals);
    const yMax = Math.max(0, ...allVals);
    const yPad = Math.max((yMax - yMin) * 0.1 || 0.1, 0.1);
    const yDomain: [number, number] = [yMin - yPad, yMax + yPad];

    const xScale = scaleLinear()
      .domain([0, Math.max(1, nSteps - 1)])
      .range([0, w]);

    const yScale = scaleLinear()
      .domain(yDomain)
      .range([h, 0]);

    const g = svg
      .attr('width', size.width)
      .attr('height', size.height)
      .append('g')
      .attr('transform', `translate(${MARGIN.left},${MARGIN.top})`);

    g.append('rect')
      .attr('width', w)
      .attr('height', h)
      .attr('fill', chartTheme.canvas)
      .attr('rx', 2);

    g.append('line')
      .attr('x1', 0)
      .attr('x2', w)
      .attr('y1', yScale(0))
      .attr('y2', yScale(0))
      .attr('stroke', chartTheme.zeroLine)
      .attr('stroke-dasharray', '2,2');

    if (lower && upper && lower.length === nSteps && upper.length === nSteps) {
      const areaData: { step: number; low: number; high: number }[] = series.map((d, i) => ({
        step: d.step,
        low: lower[i] ?? d.value,
        high: upper[i] ?? d.value,
      }));
      const lowerLine = areaData.map((d) => `L ${xScale(d.step)} ${yScale(d.low)}`).join(' ');
      const upperLine = [...areaData].reverse().map((d) => `L ${xScale(d.step)} ${yScale(d.high)}`).join(' ');
      const areaPath = `M ${xScale(areaData[0].step)} ${yScale(areaData[0].low)} ${lowerLine} ${upperLine} Z`;
      g.append('path')
        .attr('d', areaPath)
        .attr('fill', plotColor)
        .attr('fill-opacity', 0.2)
        .attr('stroke', 'none');
    }

    const pathGen = line<{ step: number; value: number }>()
      .x((d) => xScale(d.step))
      .y((d) => yScale(d.value));

    g.append('path')
      .attr('d', pathGen(series)!)
      .attr('fill', 'none')
      .attr('stroke', plotColor)
      .attr('stroke-width', 1.5)
      .attr('stroke-linecap', 'round')
      .attr('stroke-linejoin', 'round');

    g.append('text')
      .attr('x', w / 2)
      .attr('y', -6)
      .attr('text-anchor', 'middle')
      .attr('fill', chartTheme.tick)
      .attr('font-size', '10px')
      .text(title);

    g.append('g')
      .attr('transform', `translate(0,${h})`)
      .call(axisBottom(xScale).ticks(Math.min(5, nSteps)).tickSize(0))
      .call((sel) => {
        sel.select('.domain').attr('stroke', chartTheme.axis);
        sel.selectAll('.tick text').attr('fill', chartTheme.label).attr('font-size', '9px');
      });

    g.append('g')
      .call(axisLeft(yScale).ticks(3).tickSize(-w))
      .call((sel) => {
        sel.select('.domain').attr('stroke', chartTheme.axis);
        sel.selectAll('.tick line').attr('stroke', chartTheme.grid);
        sel.selectAll('.tick text').attr('fill', chartTheme.label).attr('font-size', '9px');
      });

    const tipEl = tooltipRef.current;
    g.append('rect')
      .attr('width', w)
      .attr('height', h)
      .attr('fill', 'transparent')
      .style('cursor', 'pointer')
      .on('mousemove', function (event) {
        if (!tipEl) return;
        const rect = (event.currentTarget as SVGRectElement).getBoundingClientRect();
        const mx = event.clientX - rect.left;
        const stepIdx = Math.round((mx / w) * (nSteps - 1));
        const step = Math.max(0, Math.min(stepIdx, nSteps - 1));
        const val = series[step]?.value ?? 0;
        const loStr = lower?.[step] != null ? `<br/>95% CI: [${(lower[step] ?? 0).toFixed(4)}, ${(upper?.[step] ?? 0).toFixed(4)}]` : '';
        tipEl.style.opacity = '1';
        tipEl.innerHTML =
          `<div style="font-size:11px;line-height:1.6">` +
          `<b>${title}</b><br/>` +
          `step: ${step}<br/>` +
          `value: ${val.toFixed(6)}` +
          loStr +
          `</div>`;
        const containerRect = containerRef.current!.getBoundingClientRect();
        const tipW = tipEl.offsetWidth;
        let left = event.clientX - containerRect.left - tipW / 2;
        left = Math.max(4, Math.min(left, containerRect.width - tipW - 4));
        const above = event.clientY - containerRect.top - tipEl.offsetHeight - 8;
        const below = event.clientY - containerRect.top + 8;
        tipEl.style.left = `${left}px`;
        tipEl.style.top = above > 0 ? `${above}px` : `${below}px`;
      })
      .on('mouseleave', hideTooltip);
  }, [series, lower, upper, title, size, hideTooltip, chartTheme, plotColor]);

  return (
    <div ref={containerRef} className="relative w-full h-full min-h-0 rounded-lg border border-border overflow-hidden" style={{ backgroundColor: chartTheme.canvas }}>
      <svg ref={svgRef} style={{ width: '100%', height: '100%' }} />
      <div
        ref={tooltipRef}
        className="pointer-events-none absolute z-10 rounded-md bg-popover border border-border px-3 py-2 shadow-lg transition-opacity duration-100"
        style={{ opacity: 0 }}
      />
    </div>
  );
};

export default IRFChartSingle;
