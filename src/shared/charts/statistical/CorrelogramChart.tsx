import { useEffect, useRef } from "react";
import { axisBottom, axisLeft, scaleBand, scaleLinear, select } from "d3";
import { resolveChartBox } from "@/shared/charts/core/domain";
import { joinCartesianLayers, styleChartAxis } from "@/shared/charts/core/layers";
import { useChartTheme } from "@/shared/charts/core/theme";
import {
  attachMarkTooltip,
  type D3Onable,
  PlotTooltipController,
  tooltipMutedLine,
  tooltipStrongLine,
} from "@/shared/charts/core/tooltip";
import type { ChartMargin } from "@/shared/charts/core/types";
import { useChartContainerSize } from "@/shared/charts/core/useChartContainerSize";
import { type CorrelogramBarDTO, hasLjungBoxStats } from "@/shared/types/report/correlogram";

const CORRELOGRAM_MARGIN: ChartMargin = {
  top: 28,
  right: 24,
  bottom: 36,
  left: 52,
};

const CHART_TOOLTIP_CLASS =
  "absolute pointer-events-none rounded px-2 py-1.5 bg-popover text-popover-foreground border border-border shadow-lg opacity-0 transition-opacity duration-100 z-10 text-[10px] leading-relaxed whitespace-nowrap";

interface ConfidenceRegion {
  lower: number;
  upper: number;
}

interface ConfidenceReference {
  bound: "upper" | "lower";
  value: number;
}

export interface CorrelogramChartProps {
  data: CorrelogramBarDTO[];
  ciHalfWidth: number;
  title?: string;
  color?: string;
  valueLabel?: string;
}

function formatPValueDisplay(pValue: number): string {
  return pValue < 0.0001 ? pValue.toExponential(2) : pValue.toFixed(4);
}

export function CorrelogramChart({
  data,
  ciHalfWidth,
  title,
  color,
  valueLabel = "Value",
}: CorrelogramChartProps) {
  const svgRef = useRef<SVGSVGElement>(null);
  const tooltipRef = useRef<HTMLDivElement>(null);
  const { containerRef, size } = useChartContainerSize();
  const { colors: chartTheme, series: seriesColors } = useChartTheme();
  const plotColor = color ?? seriesColors.primary;

  useEffect(() => {
    const svgNode = svgRef.current;
    if (!svgNode) return;

    const svg = select(svgNode);
    const layers = joinCartesianLayers(svg);
    const container = containerRef.current;
    const tooltip = new PlotTooltipController(tooltipRef.current, container);
    const box = resolveChartBox(size.width, size.height, CORRELOGRAM_MARGIN);

    svg
      .attr("width", size.width)
      .attr("height", size.height)
      .attr("role", "group")
      .attr("aria-label", title ?? `${valueLabel} correlogram`);

    tooltip.hide();
    if (!container || data.length === 0 || !box) {
      layers.root.attr("display", "none");
      layers.grid
        .selectAll<SVGRectElement, ConfidenceRegion>("rect.correlogram-confidence-region")
        .data([])
        .join("rect");
      layers.grid
        .selectAll<SVGLineElement, ConfidenceReference>("line.correlogram-confidence-reference")
        .data([])
        .join("line");
      layers.marks
        .selectAll<SVGRectElement, CorrelogramBarDTO>("rect.correlogram-bar")
        .data([], (bar) => String(bar.lag))
        .join("rect");
      return;
    }

    layers.root
      .attr("display", null)
      .attr("transform", `translate(${CORRELOGRAM_MARGIN.left},${CORRELOGRAM_MARGIN.top})`);
    layers.grid.attr("display", null);
    layers.xAxis.attr("display", null);
    layers.yAxis.attr("display", null);
    layers.marks.attr("display", null);
    layers.labels.attr("display", null);

    const xBand = scaleBand()
      .domain(data.map((bar) => String(bar.lag)))
      .range([0, box.plotWidth])
      .padding(0.25);
    const yExtent = Math.max(1, Math.abs(ciHalfWidth) * 1.2);
    const yScale = scaleLinear().domain([-yExtent, yExtent]).range([box.plotHeight, 0]);
    const zeroY = yScale(0);

    const confidenceRegion: ConfidenceRegion = {
      lower: Math.min(-ciHalfWidth, ciHalfWidth),
      upper: Math.max(-ciHalfWidth, ciHalfWidth),
    };
    layers.grid
      .selectAll<SVGRectElement, ConfidenceRegion>("rect.correlogram-confidence-region")
      .data([confidenceRegion])
      .join("rect")
      .attr("class", "correlogram-confidence-region")
      .attr("data-chart-region", "confidence")
      .attr("x", 0)
      .attr("y", (region) => yScale(region.upper))
      .attr("width", box.plotWidth)
      .attr("height", (region) => yScale(region.lower) - yScale(region.upper))
      .attr("fill", chartTheme.grid)
      .attr("opacity", 0.5);

    const confidenceReferences: ConfidenceReference[] = [
      { bound: "upper", value: ciHalfWidth },
      { bound: "lower", value: -ciHalfWidth },
    ];
    layers.grid
      .selectAll<SVGLineElement, ConfidenceReference>("line.correlogram-confidence-reference")
      .data(confidenceReferences, (reference) => reference.bound)
      .join("line")
      .attr("class", "correlogram-confidence-reference")
      .attr("data-chart-reference", "confidence")
      .attr("data-ci-bound", (reference) => reference.bound)
      .attr("data-chart-value", (reference) => reference.value)
      .attr("x1", 0)
      .attr("x2", box.plotWidth)
      .attr("y1", (reference) => yScale(reference.value))
      .attr("y2", (reference) => yScale(reference.value))
      .attr("stroke", chartTheme.zeroLine)
      .attr("stroke-dasharray", "4,4");

    layers.grid
      .selectAll<SVGLineElement, number>("line.correlogram-zero-reference")
      .data([0])
      .join("line")
      .attr("class", "correlogram-zero-reference")
      .attr("data-chart-reference", "zero")
      .attr("data-chart-value", 0)
      .attr("x1", 0)
      .attr("x2", box.plotWidth)
      .attr("y1", zeroY)
      .attr("y2", zeroY)
      .attr("stroke", chartTheme.zeroLine)
      .raise();

    const bars = layers.marks
      .selectAll<SVGRectElement, CorrelogramBarDTO>("rect.correlogram-bar")
      .data(data, (bar) => String(bar.lag))
      .join("rect")
      .attr("class", "correlogram-bar")
      .attr("data-chart-mark", "correlogram-bar")
      .attr("data-lag", (bar) => bar.lag)
      .attr("x", (bar) => xBand(String(bar.lag)) ?? 0)
      .attr("y", (bar) => Math.min(zeroY, yScale(bar.value)))
      .attr("width", xBand.bandwidth())
      .attr("height", (bar) => Math.max(1, Math.abs(zeroY - yScale(bar.value))))
      .attr("fill", (bar) => (bar.value >= 0 ? plotColor : seriesColors.negative))
      .attr("fill-opacity", 0.85)
      .attr("rx", 2)
      .style("cursor", "pointer");

    const detachTooltip = attachMarkTooltip(bars as D3Onable<SVGRectElement, CorrelogramBarDTO>, {
      tooltip,
      position: "anchor",
      getHtml: (bar) =>
        tooltipStrongLine(`Lag ${bar.lag}`, chartTheme, { size: 11 }) +
        tooltipMutedLine(`${valueLabel}: ${bar.value.toFixed(4)}`, chartTheme, 11) +
        (hasLjungBoxStats(bar)
          ? tooltipMutedLine(`Q(${bar.lag}): ${bar.qStat.toFixed(4)}`, chartTheme, 11) +
            tooltipMutedLine(`p-value: ${formatPValueDisplay(bar.pValue)}`, chartTheme, 11)
          : ""),
      getAriaLabel: (bar) => {
        const label = `Lag ${bar.lag}, ${valueLabel} ${bar.value.toFixed(4)}`;
        return hasLjungBoxStats(bar)
          ? `${label}, Q(${bar.lag}) ${bar.qStat.toFixed(4)}, p-value ${formatPValueDisplay(bar.pValue)}`
          : label;
      },
      onEnter: (element) =>
        select(element)
          .attr("fill-opacity", 1)
          .attr("stroke", chartTheme.tooltipFg)
          .attr("stroke-width", 1),
      onLeave: (element) => select(element).attr("fill-opacity", 0.85).attr("stroke", "none"),
    });

    layers.xAxis
      .attr("transform", `translate(0,${box.plotHeight})`)
      .call(axisBottom(xBand).tickSize(-4));
    styleChartAxis(layers.xAxis, chartTheme);

    layers.yAxis.call(axisLeft(yScale).ticks(5).tickSize(-4));
    styleChartAxis(layers.yAxis, chartTheme);

    layers.labels
      .selectAll<SVGTextElement, string>("text.correlogram-title")
      .data(title ? [title] : [])
      .join("text")
      .attr("class", "correlogram-title")
      .attr("data-chart-label", "title")
      .attr("x", box.plotWidth / 2)
      .attr("y", -10)
      .attr("text-anchor", "middle")
      .attr("fill", chartTheme.tick)
      .attr("font-size", "12px")
      .attr("font-weight", "500")
      .text((label) => label);

    return () => detachTooltip();
  }, [chartTheme, ciHalfWidth, data, plotColor, seriesColors.negative, size, title, valueLabel]);

  return (
    <div
      ref={containerRef}
      className="relative w-full flex-1 min-h-0 overflow-hidden rounded-lg border border-border bg-card"
    >
      <svg ref={svgRef} className="h-full w-full" />
      <div ref={tooltipRef} className={CHART_TOOLTIP_CLASS} />
    </div>
  );
}

export default CorrelogramChart;
