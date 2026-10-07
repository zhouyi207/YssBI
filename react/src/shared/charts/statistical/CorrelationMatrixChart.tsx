import { useEffect, useId, useRef } from "react";
import { interpolateRdBu, scaleBand, scaleSequential, select } from "d3";
import { cn } from "@/lib/utils";
import { resolveChartBox } from "@/shared/charts/core/domain";
import { joinCartesianLayers } from "@/shared/charts/core/layers";
import { useChartTheme } from "@/shared/charts/core/theme";
import {
  attachMarkTooltip,
  type D3Onable,
  PlotTooltipController,
  tooltipMutedLine,
  tooltipStrongLine,
  tooltipTickLine,
} from "@/shared/charts/core/tooltip";
import type { ChartMargin, ChartSurfaceVariant } from "@/shared/charts/core/types";
import { useChartContainerSize } from "@/shared/charts/core/useChartContainerSize";

const CORRELATION_PLOT_MARGIN: ChartMargin = {
  top: 40,
  right: 24,
  bottom: 120,
  left: 120,
};
const CORRELATION_LEGEND_GUTTER_WIDTH = 40;

const CHART_TOOLTIP_CLASS =
  "absolute pointer-events-none rounded px-2 py-1 bg-popover text-popover-foreground border border-border shadow-lg opacity-0 transition-opacity duration-100 z-10 whitespace-nowrap";

interface CorrelationCell {
  rowIndex: number;
  columnIndex: number;
  value: number;
  pValue?: number | null;
}

interface CorrelationLabel {
  index: number;
  label: string;
}

interface GradientStop {
  offset: string;
  color: string;
}

export interface CorrelationMatrixChartProps {
  labels: string[];
  matrix: (number | null)[][];
  pMatrix?: (number | null)[][];
  height?: number;
  margin?: ChartMargin;
  surface?: ChartSurfaceVariant;
  className?: string;
}

function formatPValue(pValue: number | null | undefined): string {
  if (pValue == null || Number.isNaN(pValue)) return "unavailable";
  if (pValue < 0.001) return "p < 0.001";
  return `p = ${pValue.toFixed(3)}`;
}

export function CorrelationMatrixChart({
  labels,
  matrix,
  pMatrix,
  height: heightProp,
  margin = CORRELATION_PLOT_MARGIN,
  surface = "card",
  className,
}: CorrelationMatrixChartProps) {
  const svgRef = useRef<SVGSVGElement>(null);
  const tooltipRef = useRef<HTMLDivElement>(null);
  const reactId = useId();
  const gradientId = `correlation-gradient-${reactId.replace(/[^a-zA-Z0-9_-]/g, "")}`;
  const { containerRef, size } = useChartContainerSize();
  const { colors: chartTheme, series: seriesColors } = useChartTheme();

  useEffect(() => {
    const svgNode = svgRef.current;
    if (!svgNode) return;

    const svg = select(svgNode);
    const layers = joinCartesianLayers(svg);
    const defs = svg
      .selectAll<SVGDefsElement, string>('defs[data-chart-layer="defs"]')
      .data(["defs"])
      .join("defs")
      .attr("data-chart-layer", "defs");
    const legend = svg
      .selectAll<SVGGElement, string>('g[data-chart-layer="legend"]')
      .data(["legend"])
      .join("g")
      .attr("data-chart-layer", "legend")
      .attr("pointer-events", "none");
    const container = containerRef.current;
    const tooltip = new PlotTooltipController(tooltipRef.current, container);
    const width = size.width;
    const height = heightProp ?? size.height;
    const box = resolveChartBox(width, height, margin);
    const labelData = labels.map((label, index) => ({ index, label }));
    const matrixIsSquare =
      matrix.length === labels.length && matrix.every((row) => row.length === labels.length);

    svg
      .attr("width", width)
      .attr("height", height)
      .attr("role", "group")
      .attr("aria-label", "Correlation matrix");

    tooltip.hide();
    if (
      !container ||
      labels.length === 0 ||
      !matrixIsSquare ||
      !box ||
      box.plotWidth <= CORRELATION_LEGEND_GUTTER_WIDTH
    ) {
      layers.root.attr("display", "none");
      legend.attr("display", "none");
      layers.marks
        .selectAll<SVGRectElement, CorrelationCell>("rect.cell")
        .data([], (cell) => `${cell.rowIndex}:${cell.columnIndex}`)
        .join("rect");
      return;
    }

    const matrixAreaWidth = box.plotWidth - CORRELATION_LEGEND_GUTTER_WIDTH;
    const availableSize = Math.min(matrixAreaWidth, box.plotHeight);
    const offsetX = CORRELATION_LEGEND_GUTTER_WIDTH + (matrixAreaWidth - availableSize) / 2;
    const offsetY = (box.plotHeight - availableSize) / 2;
    const indices = labelData.map((item) => item.index);
    const xScale = scaleBand<number>().domain(indices).range([0, availableSize]).padding(0.05);
    const yScale = scaleBand<number>().domain(indices).range([0, availableSize]).padding(0.05);
    const colorScale = scaleSequential(interpolateRdBu).domain([-1, 1]);

    layers.root
      .attr("display", null)
      .attr("transform", `translate(${margin.left + offsetX},${margin.top + offsetY})`);
    layers.grid.attr("display", "none");
    layers.labels.attr("display", "none");

    const cells: CorrelationCell[] = [];
    for (let rowIndex = 0; rowIndex < labels.length; rowIndex++) {
      for (let columnIndex = 0; columnIndex < labels.length; columnIndex++) {
        const value = matrix[rowIndex]?.[columnIndex];
        if (value == null || Number.isNaN(value)) continue;
        cells.push({
          rowIndex,
          columnIndex,
          value,
          pValue: pMatrix?.[rowIndex]?.[columnIndex],
        });
      }
    }

    const cellRects = layers.marks
      .selectAll<SVGRectElement, CorrelationCell>("rect.cell")
      .data(cells, (cell) => `${cell.rowIndex}:${cell.columnIndex}`)
      .join("rect")
      .attr("class", "cell")
      .attr("data-chart-mark", "correlation-cell")
      .attr("x", (cell) => xScale(cell.columnIndex) ?? 0)
      .attr("y", (cell) => yScale(cell.rowIndex) ?? 0)
      .attr("width", xScale.bandwidth())
      .attr("height", yScale.bandwidth())
      .attr("fill", (cell) => colorScale(cell.value))
      .attr("stroke", chartTheme.grid)
      .attr("stroke-width", 0.5)
      .attr("rx", 2);

    const detachTooltip = attachMarkTooltip(
      cellRects as D3Onable<SVGRectElement, CorrelationCell>,
      {
        tooltip,
        cursorOffset: { x: 12, y: -10 },
        getHtml: (cell) =>
          tooltipMutedLine(`Row: ${labels[cell.rowIndex]}`, chartTheme) +
          tooltipMutedLine(`Column: ${labels[cell.columnIndex]}`, chartTheme) +
          tooltipStrongLine(`Coefficient: ${cell.value.toFixed(3)}`, chartTheme) +
          tooltipTickLine(`p-value: ${formatPValue(cell.pValue)}`, chartTheme),
        getAriaLabel: (cell) =>
          `${labels[cell.rowIndex]} by ${labels[cell.columnIndex]}, correlation coefficient ${cell.value.toFixed(3)}, p-value ${formatPValue(cell.pValue)}`,
        onEnter: (element) =>
          select(element).attr("stroke", seriesColors.primary).attr("stroke-width", 2),
        onLeave: (element) =>
          select(element).attr("stroke", chartTheme.grid).attr("stroke-width", 0.5),
      },
    );

    layers.xAxis
      .attr("display", null)
      .attr("transform", `translate(0,${availableSize})`)
      .selectAll<SVGTextElement, CorrelationLabel>("text")
      .data(labelData, (item) => String(item.index))
      .join("text")
      .attr("x", (item) => (xScale(item.index) ?? 0) + xScale.bandwidth() / 2)
      .attr("y", 16)
      .attr("text-anchor", "end")
      .attr("transform", (item) => {
        const x = (xScale(item.index) ?? 0) + xScale.bandwidth() / 2;
        return `rotate(-45 ${x} 16)`;
      })
      .attr("fill", chartTheme.tick)
      .attr("font-size", "10px")
      .text((item) => (item.label.length > 12 ? `${item.label.slice(0, 10)}…` : item.label));

    layers.yAxis
      .attr("display", null)
      .selectAll<SVGTextElement, CorrelationLabel>("text")
      .data(labelData, (item) => String(item.index))
      .join("text")
      .attr("x", -10)
      .attr("y", (item) => (yScale(item.index) ?? 0) + yScale.bandwidth() / 2)
      .attr("text-anchor", "end")
      .attr("dominant-baseline", "middle")
      .attr("fill", chartTheme.tick)
      .attr("font-size", "10px")
      .text((item) => (item.label.length > 12 ? `${item.label.slice(0, 10)}…` : item.label));

    const gradient = defs
      .selectAll<SVGLinearGradientElement, string>(
        'linearGradient[data-chart-gradient="correlation"]',
      )
      .data([gradientId], (id) => id)
      .join("linearGradient")
      .attr("data-chart-gradient", "correlation")
      .attr("id", (id) => id)
      .attr("x1", 0)
      .attr("x2", 0)
      .attr("y1", 1)
      .attr("y2", 0);
    const gradientStops: GradientStop[] = [
      { offset: "0%", color: colorScale(-1) },
      { offset: "50%", color: colorScale(0) },
      { offset: "100%", color: colorScale(1) },
    ];
    gradient
      .selectAll<SVGStopElement, GradientStop>("stop")
      .data(gradientStops, (stop) => stop.offset)
      .join("stop")
      .attr("offset", (stop) => stop.offset)
      .attr("stop-color", (stop) => stop.color);

    const legendWidth = 8;
    const legendHeight = Math.min(100, availableSize);
    const legendX = margin.left + 8;
    const legendY = margin.top + (box.plotHeight - legendHeight) / 2;
    legend.attr("display", null);
    legend
      .selectAll<SVGRectElement, string>('rect[data-chart-legend="correlation-scale"]')
      .data(["correlation-scale"])
      .join("rect")
      .attr("data-chart-legend", "correlation-scale")
      .attr("x", legendX)
      .attr("y", legendY)
      .attr("width", legendWidth)
      .attr("height", legendHeight)
      .attr("fill", `url(#${gradientId})`)
      .attr("rx", 2);
    legend
      .selectAll<SVGTextElement, { key: string; label: string; y: number }>(
        "text[data-chart-legend-label]",
      )
      .data(
        [
          { key: "minimum", label: "-1", y: legendY + legendHeight },
          { key: "maximum", label: "1", y: legendY },
        ],
        (item) => item.key,
      )
      .join("text")
      .attr("data-chart-legend-label", (item) => item.key)
      .attr("x", legendX + legendWidth + 6)
      .attr("y", (item) => item.y)
      .attr("text-anchor", "start")
      .attr("dominant-baseline", "middle")
      .attr("fill", chartTheme.label)
      .attr("font-size", "9px")
      .text((item) => item.label);

    return () => detachTooltip();
  }, [
    chartTheme,
    gradientId,
    heightProp,
    labels,
    margin,
    matrix,
    pMatrix,
    seriesColors.primary,
    size,
  ]);

  return (
    <div
      ref={containerRef}
      className={cn(
        "relative w-full min-h-0 overflow-hidden",
        heightProp === undefined && "h-full",
        surface === "card" && "rounded-lg border border-border bg-card",
        className,
      )}
      style={heightProp === undefined ? undefined : { height: heightProp }}
    >
      <svg ref={svgRef} />
      <div ref={tooltipRef} className={CHART_TOOLTIP_CLASS} />
    </div>
  );
}
