import React, { useEffect, useRef, useState, useCallback } from 'react';
import { select, scaleLinear, axisBottom, axisLeft, extent, line } from 'd3';
import { useChartThemeColors, useChartSeriesColors } from '@/shared/theme/chartTheme';
import { plotAxisTickFormatter } from '@/shared/plot/plotTime';
import { usePlotContainerSize } from '@/shared/plot/usePlotContainerSize';
import { Switch } from '@/components/ui/switch';
import { Label } from '@/components/ui/label';
import { ToolbarIconButton } from '@/shared/ui/ToolbarIconButton';
import { cn } from '@/lib/utils';
import { plotShellClass, plotToolbarClass, DEFAULT_PLOT_MARGIN, type PlotMargin } from './plotShellStyles';

export interface LinePoint {
  x: number;
  y: number;
}

export interface LineProps {
  data: LinePoint[];
  /** X 轴标签 */
  xLabel?: string;
  /** Y 轴标签 */
  yLabel?: string;
  /** X 轴格式: "date"=天数转日期, "datetime"=微秒转日期时间, "number"=普通数值 */
  xFormat?: 'date' | 'datetime' | 'number';
  /** Y 轴格式 */
  yFormat?: 'date' | 'datetime' | 'number';
  /** 线条颜色，默认 #569cd6 */
  color?: string;
  /** 线条宽度，默认 2 */
  strokeWidth?: number;
  /** 是否显示数据点（初始值），默认 true */
  showPoints?: boolean;
  /** 图表高度，不传则随容器填充 */
  height?: number;
  /** 图表边距 */
  margin?: PlotMargin;
  /** 嵌入编辑器工作表：无边框、无圆角、填满容器 */
  embedded?: boolean;
}

function ToolbarToggle({ checked, onChange, label }: { checked: boolean; onChange: (v: boolean) => void; label: string }) {
  return (
    <div className="flex items-center gap-2">
      <Switch id={`line-toggle-${label}`} size="sm" checked={checked} onCheckedChange={onChange} />
      <Label htmlFor={`line-toggle-${label}`} className="cursor-pointer text-[11px] text-muted-foreground">
        {label}
      </Label>
    </div>
  );
}

const Line: React.FC<LineProps> = ({
  data,
  xLabel,
  yLabel,
  xFormat = 'number',
  yFormat = 'number',
  color,
  strokeWidth = 2,
  showPoints: showPointsInit = true,
  height: heightProp,
  margin = DEFAULT_PLOT_MARGIN,
  embedded = false,
}) => {
  const svgRef = useRef<SVGSVGElement>(null);
  const { containerRef, size } = usePlotContainerSize();
  const [toolbarOpen, setToolbarOpen] = useState(false);
  const [pointsVisible, setPointsVisible] = useState(showPointsInit);
  const chartTheme = useChartThemeColors();
  const seriesColors = useChartSeriesColors();
  const plotColor = color ?? seriesColors.primary;

  const toggleToolbar = useCallback(() => setToolbarOpen((v) => !v), []);

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

    const yExtent = extent(data, (d) => d.y) as [number, number];
    const yPad = (yExtent[1] - yExtent[0]) * 0.06 || 1;
    const yScale = scaleLinear().domain([yExtent[0] - yPad, yExtent[1] + yPad]).range([h, 0]);

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
      .attr('stroke', chartTheme.grid).attr('stroke-dasharray', '2,3');

    // x axis
    const xAxis = axisBottom(xScale).ticks(6).tickSize(-4);
    const xTickFormat = plotAxisTickFormatter(xFormat);
    if (xTickFormat) xAxis.tickFormat(xTickFormat);
    g.append('g')
      .attr('transform', `translate(0,${h})`)
      .call(xAxis)
      .call((sel) => {
        sel.select('.domain').attr('stroke', chartTheme.axis);
        sel.selectAll('.tick line').attr('stroke', chartTheme.axis);
        sel.selectAll('.tick text').attr('fill', chartTheme.tick).attr('font-size', '10px');
      });

    // y axis
    const yAxis = axisLeft(yScale).ticks(5).tickSize(-4);
    const yTickFormat = plotAxisTickFormatter(yFormat);
    if (yTickFormat) yAxis.tickFormat(yTickFormat);
    g.append('g')
      .call(yAxis)
      .call((sel) => {
        sel.select('.domain').attr('stroke', chartTheme.axis);
        sel.selectAll('.tick line').attr('stroke', chartTheme.axis);
        sel.selectAll('.tick text').attr('fill', chartTheme.tick).attr('font-size', '10px');
      });

    if (xLabel) {
      g.append('text')
        .attr('x', w / 2).attr('y', h + 32)
        .attr('text-anchor', 'middle')
        .attr('fill', chartTheme.label).attr('font-size', '11px')
        .text(xLabel);
    }

    if (yLabel) {
      g.append('text')
        .attr('transform', 'rotate(-90)')
        .attr('x', -h / 2).attr('y', -42)
        .attr('text-anchor', 'middle')
        .attr('fill', chartTheme.label).attr('font-size', '11px')
        .text(yLabel);
    }

    // line path
    const pathLine = line<LinePoint>()
      .x((d) => xScale(d.x))
      .y((d) => yScale(d.y));

    g.append('path')
      .datum(data)
      .attr('d', pathLine)
      .attr('fill', 'none')
      .attr('stroke', plotColor)
      .attr('stroke-width', strokeWidth)
      .attr('stroke-linecap', 'round')
      .attr('stroke-linejoin', 'round');

    // optional points
    if (pointsVisible) {
      g.selectAll('circle')
        .data(data)
        .join('circle')
        .attr('cx', (d) => xScale(d.x))
        .attr('cy', (d) => yScale(d.y))
        .attr('r', 3)
        .attr('fill', plotColor)
        .attr('fill-opacity', 0.7)
        .attr('stroke', plotColor)
        .attr('stroke-opacity', 0.3)
        .attr('stroke-width', 1);
    }
  }, [data, xLabel, yLabel, xFormat, yFormat, plotColor, strokeWidth, pointsVisible, heightProp, margin, size, chartTheme]);

  if (embedded) {
    return (
      <div ref={containerRef} className="h-full w-full min-h-0 overflow-hidden bg-[var(--workbench-bg)]">
        <svg ref={svgRef} />
      </div>
    );
  }

  return (
    <div className={cn('flex min-h-0 w-full h-full flex-col', plotShellClass)}>
      <div className="flex items-center justify-end px-2 pt-1.5 pb-0">
        <ToolbarIconButton
          type="button"
          variant="ghost"
          size="icon-xs"
          onClick={toggleToolbar}
          tooltip="Toggle toolbar"
          className={cn(
            toolbarOpen
              ? 'text-[var(--accent-color)] bg-[var(--accent-color)]/10'
              : 'text-muted-foreground hover:text-foreground',
          )}
        >
          <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" strokeWidth={2} viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.066 2.573c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.573 1.066c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.066-2.573c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z" />
            <circle cx="12" cy="12" r="3" />
          </svg>
        </ToolbarIconButton>
      </div>

      {toolbarOpen && (
        <div className={plotToolbarClass}>
          <ToolbarToggle checked={pointsVisible} onChange={setPointsVisible} label="Scatter Points" />
        </div>
      )}

      {/* chart */}
      <div ref={containerRef} className="flex-1 min-h-0">
        <svg ref={svgRef} />
      </div>
    </div>
  );
};

export default Line;
