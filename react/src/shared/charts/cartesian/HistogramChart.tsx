import { useEffect, useRef } from "react";
import { axisBottom, axisLeft, max, scaleBand, scaleLinear, select } from "d3";
import { cn } from "@/lib/utils";
import { resolveChartBox } from "@/shared/charts/core/domain";
import {
  joinCartesianLayers,
  styleChartAxis,
  updateCartesianLabels,
  updateHorizontalGrid,
} from "@/shared/charts/core/layers";
import { DEFAULT_CARTESIAN_MARGIN } from "@/shared/charts/core/margins";
import { useChartTheme } from "@/shared/charts/core/theme";
import {
  attachMarkTooltip,
  type D3Onable,
  PlotTooltipController,
  tooltipTwoLine,
} from "@/shared/charts/core/tooltip";
import type { ChartMargin, ChartSurfaceVariant } from "@/shared/charts/core/types";
import { useChartContainerSize } from "@/shared/charts/core/useChartContainerSize";
import type { HistogramBin } from "@/shared/charts/ChartModel";

const COMPACT_HISTOGRAM_MARGIN: ChartMargin = {
  top: 4,
  right: 4,
  bottom: 4,
  left: 4,
};

const CHART_TOOLTIP_CLASS =
  "absolute pointer-events-none rounded px-2 py-1 bg-popover text-popover-foreground border border-border shadow-lg opacity-0 transition-opacity duration-100 z-10 whitespace-nowrap";

export type { HistogramBin } from "@/shared/charts/ChartModel";

export interface HistogramChartProps {
  data: HistogramBin[];
  xLabel?: string;
  yLabel?: string;
  color?: string;
  height?: number;
  margin?: ChartMargin;
  compact?: boolean;
  surface?: ChartSurfaceVariant;
  className?: string;
}

export function HistogramChart({
  data,
  xLabel,
  yLabel = "Frequency",
  color,
  height: heightProp,
  margin: marginProp,
  compact = false,
  surface = "card",
  className,
}: HistogramChartProps) {
  const svgRef = useRef<SVGSVGElement>(null);
  const tooltipRef = useRef<HTMLDivElement>(null);
  const { containerRef, size } = useChartContainerSize();
  const { colors: chartTheme, series: seriesColors } = useChartTheme();
  const plotColor = color ?? seriesColors.primary;
  const margin = marginProp ?? (compact ? COMPACT_HISTOGRAM_MARGIN : DEFAULT_CARTESIAN_MARGIN);

  useEffect(() => {
    const svgNode = svgRef.current;
    if (!svgNode) return;

    const svg = select(svgNode);
    const layers = joinCartesianLayers(svg);
    const container = containerRef.current;
    const tooltip = new PlotTooltipController(tooltipRef.current, container);
    const width = size.width;
    const height = heightProp ?? size.height;
    const box = resolveChartBox(width, height, margin);

    svg
      .attr("width", width)
      .attr("height", height)
      .attr("role", compact ? "group" : "img")
      .attr("aria-label", `${yLabel} histogram by ${xLabel ?? "bin"}`);

    tooltip.hide();
    if (!container || data.length === 0 || !box) {
      layers.root.attr("display", "none");
      layers.marks
        .selectAll<SVGRectElement, HistogramBin>("rect.bar")
        .data([], (bin) => bin.label)
        .join("rect");
      return;
    }

    layers.root.attr("display", null).attr("transform", `translate(${margin.left},${margin.top})`);
    layers.grid.attr("display", compact ? "none" : null);
    layers.xAxis.attr("display", compact ? "none" : null);
    layers.yAxis.attr("display", compact ? "none" : null);
    layers.labels.attr("display", compact ? "none" : null);

    const xBand = scaleBand()
      .domain(data.map((bin) => bin.label))
      .range([0, box.plotWidth])
      .padding(compact ? 0.04 : 0.08);
    const observedMax = max(data, (bin) => bin.count) ?? 0;
    const domainMax = observedMax > 0 ? observedMax * 1.1 : 1;
    const yScale = scaleLinear().domain([0, domainMax]).nice().range([box.plotHeight, 0]);

    updateHorizontalGrid(
      layers.grid,
      compact ? [] : yScale.ticks(5),
      (value) => yScale(value),
      box.plotWidth,
      chartTheme.grid,
    );

    if (!compact) {
      layers.xAxis
        .attr("transform", `translate(0,${box.plotHeight})`)
        .call(axisBottom(xBand).tickSize(-4));
      styleChartAxis(layers.xAxis, chartTheme);
      layers.xAxis
        .selectAll<SVGTextElement, string>(".tick text")
        .attr("text-anchor", "end")
        .attr("transform", data.length > 6 ? "rotate(-40)" : null);
      layers.yAxis.call(axisLeft(yScale).ticks(5).tickSize(-4));
      styleChartAxis(layers.yAxis, chartTheme);
      updateCartesianLabels(layers.labels, box, { x: xLabel, y: yLabel }, chartTheme.label);
    }

    const bars = layers.marks
      .selectAll<SVGRectElement, HistogramBin>("rect.bar")
      .data(data, (bin) => bin.label)
      .join("rect")
      .attr("class", "bar")
      .attr("data-chart-mark", "histogram-bar")
      .attr("x", (bin) => xBand(bin.label) ?? 0)
      .attr("y", (bin) => yScale(bin.count))
      .attr("width", xBand.bandwidth())
      .attr("height", (bin) => box.plotHeight - yScale(bin.count))
      .attr("fill", plotColor)
      .attr("fill-opacity", 0.75)
      .attr("stroke", plotColor)
      .attr("stroke-opacity", 0.4)
      .attr("stroke-width", 0.5)
      .attr("rx", compact ? 1 : 0);

    if (!compact) {
      bars.attr("tabindex", null).attr("aria-label", null);
      return;
    }

    const detachTooltip = attachMarkTooltip(bars as D3Onable<SVGRectElement, HistogramBin>, {
      tooltip,
      getHtml: (bin) => tooltipTwoLine(chartTheme, bin.label, String(bin.count), plotColor),
      getAriaLabel: (bin) => `Histogram bin ${bin.label}, ${yLabel} ${bin.count}`,
      onEnter: (element) => select(element).attr("fill-opacity", 1),
      onLeave: (element) => select(element).attr("fill-opacity", 0.75),
    });

    return () => detachTooltip();
  }, [chartTheme, compact, data, heightProp, margin, plotColor, size, xLabel, yLabel]);

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
      {compact && <div ref={tooltipRef} className={CHART_TOOLTIP_CLASS} />}
    </div>
  );
}
