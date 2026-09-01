import { useEffect, useMemo, useRef } from "react";
import { axisBottom, axisLeft, line, scaleLinear, select } from "d3";
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
import type { XYPoint } from "@/shared/charts/ChartModel";

export interface MultiLineSeries {
  id: string;
  label: string;
  points: XYPoint[];
  color?: string;
}

export interface MultiLineChartProps {
  series: MultiLineSeries[];
  xLabel?: string;
  yLabel?: string;
  height?: number;
  margin?: ChartMargin;
  xDomain?: [number, number];
  yDomain?: [number, number];
  showLegend?: boolean;
  className?: string;
}

export function MultiLineChart({
  series,
  xLabel,
  yLabel,
  height = 224,
  margin = DEFAULT_CARTESIAN_MARGIN,
  xDomain,
  yDomain,
  showLegend = true,
  className,
}: MultiLineChartProps) {
  const svgRef = useRef<SVGSVGElement>(null);
  const { containerRef, size } = useChartContainerSize();
  const { colors: chartTheme, series: themeSeries } = useChartTheme();
  const visibleSeries = useMemo(() => series.filter((item) => item.points.length > 0), [series]);
  const colors = themeSeries.palette;

  useEffect(() => {
    const svgNode = svgRef.current;
    if (!svgNode) return;

    const svg = select(svgNode);
    const layers = joinCartesianLayers(svg);
    const box = resolveChartBox(size.width, height, margin);

    svg
      .attr("width", size.width)
      .attr("height", height)
      .attr("role", "img")
      .attr("aria-label", `${yLabel ?? "value"} by ${xLabel ?? "x"}`);

    if (visibleSeries.length === 0 || !box) {
      layers.root.attr("display", "none");
      layers.marks
        .selectAll<SVGPathElement, MultiLineSeries>('path[data-chart-mark="series"]')
        .data([], (item) => item.id)
        .join("path");
      return;
    }

    layers.root.attr("display", null).attr("transform", `translate(${margin.left},${margin.top})`);

    const points = visibleSeries.flatMap((item) => item.points);
    const resolvedXDomain =
      xDomain ??
      paddedNumericDomain(
        points.map((point) => point.x),
        0.04,
        1,
      );
    const resolvedYDomain =
      yDomain ??
      paddedNumericDomain(
        points.map((point) => point.y),
        0.04,
        1,
      );
    const xScale = scaleLinear().domain(resolvedXDomain).range([0, box.plotWidth]);
    const yScale = scaleLinear().domain(resolvedYDomain).range([box.plotHeight, 0]);

    updateHorizontalGrid(
      layers.grid,
      yScale.ticks(5),
      (value) => yScale(value),
      box.plotWidth,
      chartTheme.grid,
    );
    layers.xAxis
      .attr("transform", `translate(0,${box.plotHeight})`)
      .call(axisBottom(xScale).ticks(6).tickSize(-4));
    styleChartAxis(layers.xAxis, chartTheme);
    layers.yAxis.call(axisLeft(yScale).ticks(5).tickSize(-4));
    styleChartAxis(layers.yAxis, chartTheme);
    updateCartesianLabels(layers.labels, box, { x: xLabel, y: yLabel }, chartTheme.label);

    const seriesLine = line<XYPoint>()
      .defined((point) => Number.isFinite(point.x) && Number.isFinite(point.y))
      .x((point) => xScale(point.x))
      .y((point) => yScale(point.y));

    layers.marks
      .selectAll<SVGPathElement, MultiLineSeries>('path[data-chart-mark="series"]')
      .data(visibleSeries, (item) => item.id)
      .join("path")
      .attr("data-chart-mark", "series")
      .attr("d", (item) => seriesLine(item.points))
      .attr("fill", "none")
      .attr("stroke", (item, index) => item.color ?? colors[index % colors.length])
      .attr("stroke-width", 1.8)
      .attr("stroke-linecap", "round")
      .attr("stroke-linejoin", "round");
  }, [
    chartTheme,
    colors,
    height,
    margin,
    size.width,
    visibleSeries,
    xDomain,
    xLabel,
    yDomain,
    yLabel,
  ]);

  return (
    <div className={cn("w-full space-y-2", className)} hidden={visibleSeries.length === 0}>
      <div
        ref={containerRef}
        className="w-full overflow-hidden rounded-md border border-border bg-muted/10"
        style={{ height }}
      >
        <svg ref={svgRef} />
      </div>
      {showLegend ? (
        <div
          className="flex flex-wrap gap-3 text-xs text-muted-foreground"
          aria-label="Chart legend"
        >
          {visibleSeries.map((item, index) => (
            <span key={item.id} className="inline-flex items-center gap-1">
              <span
                className="size-2 rounded-full"
                style={{ backgroundColor: item.color ?? colors[index % colors.length] }}
              />
              {item.label}
            </span>
          ))}
        </div>
      ) : null}
    </div>
  );
}
