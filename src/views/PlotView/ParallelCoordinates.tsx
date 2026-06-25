import React, { useEffect, useRef, useState } from 'react';
import { select, scaleLinear, scalePoint, line, extent } from 'd3';
import { useChartThemeColors, useChartSeriesColors } from '@/shared/theme/chartTheme';
import { cn } from '@/lib/utils';
import { plotContainerClass, plotTooltipRichClass } from './plotShellStyles';

export interface ParallelAxis {
  name: string;
  type: 'number' | 'string';
}

export interface ParallelCoordinatesProps {
  /** 列定义 */
  axes: ParallelAxis[];
  /** 行数据，每行与 axes 等长 */
  rows: (string | number | null)[][];
  /** 线条颜色，默认 #569cd6 */
  color?: string;
  /** 最大绘制行数（采样），默认 200 */
  maxLines?: number;
}

const MARGIN = { top: 28, right: 16, bottom: 12, left: 16 };
const AXIS_LABEL_SIZE = 10;

const ParallelCoordinates: React.FC<ParallelCoordinatesProps> = ({
  axes,
  rows,
  color,
  maxLines = 200,
}) => {
  const containerRef = useRef<HTMLDivElement>(null);
  const svgRef = useRef<SVGSVGElement>(null);
  const tooltipRef = useRef<HTMLDivElement>(null);
  const [size, setSize] = useState({ width: 0, height: 0 });
  const chartTheme = useChartThemeColors();
  const seriesColors = useChartSeriesColors();
  const plotColor = color ?? seriesColors.primary;

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
    if (!container || axes.length === 0 || rows.length === 0 || size.width === 0 || size.height === 0) return;

    const width = size.width;
    const height = size.height;

    const w = width - MARGIN.left - MARGIN.right;
    const h = height - MARGIN.top - MARGIN.bottom;

    const sampled = rows.length > maxLines
      ? Array.from({ length: maxLines }, (_, i) => rows[Math.floor(i * rows.length / maxLines)])
      : rows;

    const xScale = scalePoint()
      .domain(axes.map((_, i) => String(i)))
      .range([0, w]);

    type YScale = ((v: any) => number | undefined);
    const yScales: YScale[] = axes.map((axis, colIdx) => {
      if (axis.type === 'number') {
        const vals = rows.map(r => r[colIdx]).filter(v => v != null) as number[];
        const [lo, hi] = extent(vals) as [number, number];
        const pad = (hi - lo) * 0.05 || 1;
        return scaleLinear().domain([lo - pad, hi + pad]).range([h, 0]) as unknown as YScale;
      } else {
        const unique = [...new Set(rows.map(r => r[colIdx]).filter(v => v != null).map(String))];
        return scalePoint().domain(unique).range([h, 0]).padding(0.1) as unknown as YScale;
      }
    });

    const g = svg
      .attr('width', width)
      .attr('height', height)
      .append('g')
      .attr('transform', `translate(${MARGIN.left},${MARGIN.top})`);

    // axes
    axes.forEach((axis, i) => {
      const x = xScale(String(i))!;

      g.append('line')
        .attr('x1', x).attr('x2', x)
        .attr('y1', 0).attr('y2', h)
        .attr('stroke', chartTheme.axis).attr('stroke-width', 1);

      g.append('text')
        .attr('x', x).attr('y', -10)
        .attr('text-anchor', 'middle')
        .attr('fill', chartTheme.tick)
        .attr('font-size', `${AXIS_LABEL_SIZE}px`)
        .attr('font-weight', '600')
        .text(axis.name.length > 10 ? axis.name.slice(0, 9) + '…' : axis.name);

      if (axis.type === 'number') {
        const sc = yScales[i] as unknown as ReturnType<typeof scaleLinear>;
        const ticks = sc.ticks ? sc.ticks(4) : [];
        ticks.forEach((t: number) => {
          const y = sc(t) as number | undefined;
          if (y == null) return;
          g.append('text')
            .attr('x', x - 4).attr('y', y + 3)
            .attr('text-anchor', 'end')
            .attr('fill', chartTheme.tick)
            .attr('font-size', '8px')
            .text(Number.isInteger(t) ? String(t) : t.toFixed(1));
        });
      }
    });

    const tooltip = select(tooltipRef.current);

    const pathGen = line<{ x: number; y: number }>()
      .defined(d => d.y != null && !isNaN(d.y))
      .x(d => d.x)
      .y(d => d.y);

    // lines
    const linesGroup = g.append('g');
    sampled.forEach((row, rowIdx) => {
      const points = axes.map((_, colIdx) => {
        const x = xScale(String(colIdx))!;
        const val = row[colIdx];
        if (val == null) return { x, y: NaN };
        const y = (yScales[colIdx] as any)(axis_val(axes[colIdx], val));
        return { x, y: y ?? NaN };
      });

      linesGroup.append('path')
        .datum({ points, row, rowIdx })
        .attr('d', (d: any) => pathGen(d.points))
        .attr('fill', 'none')
        .attr('stroke', plotColor)
        .attr('stroke-opacity', Math.min(0.35, 60 / sampled.length))
        .attr('stroke-width', 1.2)
        .on('mouseenter', function (_, d: any) {
          select(this)
            .attr('stroke-opacity', 1)
            .attr('stroke-width', 2.5)
            .raise();
          const html = axes.map((a, ci) => {
            const v = d.row[ci];
            return `<span style="color:${chartTheme.tooltipMuted}">${a.name}:</span> <span style="color:${chartTheme.tooltipFg}">${v ?? 'null'}</span>`;
          }).join('<br/>');
          tooltip.style('opacity', '1').html(html);
        })
        .on('mousemove', function (event) {
          const rect = container!.getBoundingClientRect();
          tooltip
            .style('left', `${event.clientX - rect.left + 12}px`)
            .style('top', `${event.clientY - rect.top - 10}px`);
        })
        .on('mouseleave', function () {
          select(this)
            .attr('stroke-opacity', Math.min(0.35, 60 / sampled.length))
            .attr('stroke-width', 1.2);
          tooltip.style('opacity', '0');
        });
    });
  }, [axes, rows, plotColor, maxLines, size, chartTheme]);

  return (
    <div ref={containerRef} className={cn(plotContainerClass())}>
      <svg ref={svgRef} className="w-full h-full" />
      <div ref={tooltipRef} className={plotTooltipRichClass} />
    </div>
  );
};

function axis_val(axis: ParallelAxis, val: string | number | null): any {
  if (val == null) return val;
  return axis.type === 'number' ? Number(val) : String(val);
}

export default ParallelCoordinates;
