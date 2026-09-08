import { useEffect, useRef } from "react";
import { area, axisBottom, axisLeft, line, scaleLinear, select } from "d3";
import { cn } from "@/lib/utils";
import { paddedNumericDomain, resolveChartBox } from "@/shared/charts/core/domain";
import {
  joinCartesianLayers,
  styleChartAxis,
  updateCartesianLabels,
  updateHorizontalGrid,
} from "@/shared/charts/core/layers";
import { DEFAULT_CARTESIAN_MARGIN } from "@/shared/charts/core/margins";
import { useChartTheme } from "@/shared/charts/core/theme";
import type { ChartMargin } from "@/shared/charts/core/types";
import { useChartContainerSize } from "@/shared/charts/core/useChartContainerSize";

export interface PredictiveIntervalPoint {
  observation: number;
  observed: number;
  mean: number;
  lower: number;
  upper: number;
}

export interface PredictiveIntervalChartProps {
  data: PredictiveIntervalPoint[];
  xLabel?: string;
  yLabel?: string;
  height?: number;
  margin?: ChartMargin;
  className?: string;
}

export function PredictiveIntervalChart({
  data,
  xLabel = "observation",
  yLabel = "value",
  height = 280,
  margin = DEFAULT_CARTESIAN_MARGIN,
  className,
}: PredictiveIntervalChartProps) {
  const svgRef = useRef<SVGSVGElement>(null);
  const { containerRef, size } = useChartContainerSize();
  const { colors: chartTheme, series: seriesColors } = useChartTheme();

  useEffect(() => {
    const svgNode = svgRef.current;
    if (!svgNode) return;

    const svg = select(svgNode);
    const layers = joinCartesianLayers(svg);
    const intervalLayer = layers.marks
      .selectAll<SVGGElement, string>('g[data-chart-mark-layer="interval"]')
      .data(["interval"], (layer) => layer)
      .join("g")
      .attr("data-chart-mark-layer", "interval");
    const meanLayer = layers.marks
      .selectAll<SVGGElement, string>('g[data-chart-mark-layer="mean"]')
      .data(["mean"], (layer) => layer)
      .join("g")
      .attr("data-chart-mark-layer", "mean");
    const observedLayer = layers.marks
      .selectAll<SVGGElement, string>('g[data-chart-mark-layer="observed"]')
      .data(["observed"], (layer) => layer)
      .join("g")
      .attr("data-chart-mark-layer", "observed");
    const sorted = [...data]
      .filter((point) =>
        [point.observation, point.observed, point.mean, point.lower, point.upper].every(
          Number.isFinite,
        ),
      )
      .sort((left, right) => left.observation - right.observation);
    const box = resolveChartBox(size.width, height, margin);

    svg
      .attr("width", size.width)
      .attr("height", height)
      .attr("role", "img")
      .attr("aria-label", `${yLabel} posterior predictive interval by ${xLabel}`);

    if (sorted.length === 0 || !box) {
      layers.root.attr("display", "none");
      intervalLayer
        .selectAll<SVGPathElement, PredictiveIntervalPoint[]>("path")
        .data([], () => "interval")
        .join("path");
      meanLayer
        .selectAll<SVGPathElement, PredictiveIntervalPoint[]>("path")
        .data([], () => "mean")
        .join("path");
      observedLayer
        .selectAll<SVGCircleElement, PredictiveIntervalPoint>("circle")
        .data([], (point) => String(point.observation))
        .join("circle");
      return;
    }

    layers.root.attr("display", null).attr("transform", `translate(${margin.left},${margin.top})`);

    const xDomain = paddedNumericDomain(
      sorted.map((point) => point.observation),
      0.01,
      1,
    );
    const yDomain = paddedNumericDomain(
      sorted.flatMap((point) => [point.lower, point.upper, point.observed]),
      0.06,
      1,
    );
    const xScale = scaleLinear().domain(xDomain).range([0, box.plotWidth]);
    const yScale = scaleLinear().domain(yDomain).range([box.plotHeight, 0]);

    updateHorizontalGrid(
      layers.grid,
      yScale.ticks(5),
      (value) => yScale(value),
      box.plotWidth,
      chartTheme.grid,
    );
    layers.xAxis
      .attr("transform", `translate(0,${box.plotHeight})`)
      .call(axisBottom(xScale).ticks(8).tickSize(-4));
    styleChartAxis(layers.xAxis, chartTheme);
    layers.yAxis.call(axisLeft(yScale).ticks(5).tickSize(-4));
    styleChartAxis(layers.yAxis, chartTheme);
    updateCartesianLabels(layers.labels, box, { x: xLabel, y: yLabel }, chartTheme.label);

    const intervalArea = area<PredictiveIntervalPoint>()
      .x((point) => xScale(point.observation))
      .y0((point) => yScale(point.lower))
      .y1((point) => yScale(point.upper));
    const meanLine = line<PredictiveIntervalPoint>()
      .x((point) => xScale(point.observation))
      .y((point) => yScale(point.mean));

    intervalLayer
      .selectAll<SVGPathElement, PredictiveIntervalPoint[]>("path")
      .data([sorted], () => "interval")
      .join("path")
      .attr("data-chart-mark", "interval")
      .attr("d", (points) => intervalArea(points))
      .attr("fill", seriesColors.primary)
      .attr("fill-opacity", 0.18);
    meanLayer
      .selectAll<SVGPathElement, PredictiveIntervalPoint[]>("path")
      .data([sorted], () => "mean")
      .join("path")
      .attr("data-chart-mark", "mean")
      .attr("d", (points) => meanLine(points))
      .attr("fill", "none")
      .attr("stroke", seriesColors.primary)
      .attr("stroke-width", 1.8)
      .attr("stroke-linecap", "round")
      .attr("stroke-linejoin", "round");
    const observedMarks = observedLayer
      .selectAll<SVGCircleElement, PredictiveIntervalPoint>("circle")
      .data(sorted, (point) => String(point.observation))
      .join("circle")
      .attr("data-chart-mark", "observed")
      .attr("cx", (point) => xScale(point.observation))
      .attr("cy", (point) => yScale(point.observed))
      .attr("r", 2.4)
      .attr("fill", (point) =>
        point.observed < point.lower || point.observed > point.upper
          ? seriesColors.highlight
          : seriesColors.secondary,
      )
      .attr("stroke", chartTheme.canvas)
      .attr("stroke-width", 0.7);

    observedMarks
      .selectAll<SVGTitleElement, PredictiveIntervalPoint>("title")
      .data((point) => [point])
      .join("title")
      .text(
        (point) =>
          `observation ${point.observation}\nobserved: ${point.observed}\nmean: ${point.mean}\n95% interval: [${point.lower}, ${point.upper}]`,
      );
  }, [chartTheme, data, height, margin, seriesColors, size.width, xLabel, yLabel]);

  return (
    <div className={cn("space-y-2", className)}>
      <div
        ref={containerRef}
        className="w-full overflow-hidden rounded-md border border-border bg-muted/10"
        style={{ height }}
      >
        <svg ref={svgRef} />
      </div>
      <div className="flex flex-wrap gap-4 text-xs text-muted-foreground" aria-label="Chart legend">
        <span className="inline-flex items-center gap-1">
          <span
            className="h-2 w-4 rounded-sm"
            style={{ backgroundColor: seriesColors.primary, opacity: 0.3 }}
          />
          95% predictive interval
        </span>
        <span className="inline-flex items-center gap-1">
          <span className="h-0.5 w-4" style={{ backgroundColor: seriesColors.primary }} />
          Predictive mean
        </span>
        <span className="inline-flex items-center gap-1">
          <span
            className="size-2 rounded-full"
            style={{ backgroundColor: seriesColors.secondary }}
          />
          Observed
        </span>
        <span className="inline-flex items-center gap-1">
          <span
            className="size-2 rounded-full"
            style={{ backgroundColor: seriesColors.highlight }}
          />
          Outside interval
        </span>
      </div>
    </div>
  );
}
