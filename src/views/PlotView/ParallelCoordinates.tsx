import React, { useEffect, useRef } from 'react';
import { select, scalePoint, line } from 'd3';
import { useChartThemeColors, useChartSeriesColors } from '@/shared/theme/chartTheme';
import { usePlotContainerSize } from '@/shared/plot/usePlotContainerSize';
import {
  attachHoverTooltip,
  type D3Onable,
  PlotTooltipController,
  escapeTooltipHtml,
  tooltipRichBlock,
} from '@/shared/plot/d3Tooltip';
import {
  columnAxisKindFromType,
  createColumnAxisScale,
  mapColumnAxisValue,
  numericColumnAxisTicks,
  type ColumnAxisScale,
} from '@/shared/plot/axisScale';
import { cn } from '@/lib/utils';
import { PARALLEL_COORDINATES_MARGIN, plotContainerClass, plotTooltipRichClass } from './plotShellStyles';

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

const AXIS_LABEL_SIZE = 10;

interface ParallelLineDatum {
  points: { x: number; y: number }[];
  row: (string | number | null)[];
  rowIdx: number;
}

const ParallelCoordinates: React.FC<ParallelCoordinatesProps> = ({
  axes,
  rows,
  color,
  maxLines = 200,
}) => {
  const svgRef = useRef<SVGSVGElement>(null);
  const tooltipRef = useRef<HTMLDivElement>(null);
  const { containerRef, size } = usePlotContainerSize();
  const chartTheme = useChartThemeColors();
  const seriesColors = useChartSeriesColors();
  const plotColor = color ?? seriesColors.primary;

  useEffect(() => {
    const svg = select(svgRef.current);
    svg.selectAll('*').remove();

    const container = containerRef.current;
    if (!container || axes.length === 0 || rows.length === 0 || size.width === 0 || size.height === 0) return;

    const width = size.width;
    const height = size.height;

    const w = width - PARALLEL_COORDINATES_MARGIN.left - PARALLEL_COORDINATES_MARGIN.right;
    const h = height - PARALLEL_COORDINATES_MARGIN.top - PARALLEL_COORDINATES_MARGIN.bottom;

    const sampled = rows.length > maxLines
      ? Array.from({ length: maxLines }, (_, i) => rows[Math.floor(i * rows.length / maxLines)])
      : rows;

    const xScale = scalePoint()
      .domain(axes.map((_, i) => String(i)))
      .range([0, w]);

    const yScales: ColumnAxisScale[] = axes.map((axis, colIdx) => {
      const colValues = rows.map((r) => r[colIdx]);
      return createColumnAxisScale(
        columnAxisKindFromType(axis.type),
        colValues,
        [h, 0],
      );
    });

    const g = svg
      .attr('width', width)
      .attr('height', height)
      .append('g')
      .attr('transform', `translate(${PARALLEL_COORDINATES_MARGIN.left},${PARALLEL_COORDINATES_MARGIN.top})`);

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

      const axisScale = yScales[i];
      if (axisScale.kind === 'numeric') {
        numericColumnAxisTicks(axisScale, 4).forEach((t) => {
          const y = axisScale.scale(t);
          if (y == null || Number.isNaN(y)) return;
          g.append('text')
            .attr('x', x - 4).attr('y', y + 3)
            .attr('text-anchor', 'end')
            .attr('fill', chartTheme.tick)
            .attr('font-size', '8px')
            .text(Number.isInteger(t) ? String(t) : t.toFixed(1));
        });
      }
    });

    const tip = new PlotTooltipController(tooltipRef.current, container);
    const lineOpacity = Math.min(0.35, 60 / sampled.length);

    const pathGen = line<{ x: number; y: number }>()
      .defined((d) => d.y != null && !Number.isNaN(d.y))
      .x((d) => d.x)
      .y((d) => d.y);

    const linesGroup = g.append('g');
    sampled.forEach((row, rowIdx) => {
      const points = axes.map((_, colIdx) => {
        const x = xScale(String(colIdx))!;
        const val = row[colIdx];
        if (val == null) return { x, y: NaN };
        const y = mapColumnAxisValue(yScales[colIdx], val);
        return { x, y: y ?? NaN };
      });

      const path = linesGroup
        .append('path')
        .datum({ points, row, rowIdx } satisfies ParallelLineDatum)
        .attr('d', (d) => pathGen(d.points) ?? '')
        .attr('fill', 'none')
        .attr('stroke', plotColor)
        .attr('stroke-opacity', lineOpacity)
        .attr('stroke-width', 1.2);

      attachHoverTooltip(path as D3Onable<SVGPathElement, ParallelLineDatum>, {
        tooltip: tip,
        cursorOffset: { x: 12, y: -10 },
        getHtml: (d) => {
          const inner = axes
            .map((a, ci) => {
              const v = d.row[ci];
              const display = v == null ? 'null' : String(v);
              return `<span style="color:${chartTheme.tooltipMuted}">${escapeTooltipHtml(a.name)}:</span> <span style="color:${chartTheme.tooltipFg}">${escapeTooltipHtml(display)}</span>`;
            })
            .join('<br/>');
          return tooltipRichBlock(inner, chartTheme);
        },
        onEnter: (el) => {
          select(el).attr('stroke-opacity', 1).attr('stroke-width', 2.5).raise();
        },
        onLeave: (el) => {
          select(el).attr('stroke-opacity', lineOpacity).attr('stroke-width', 1.2);
        },
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

export default ParallelCoordinates;
