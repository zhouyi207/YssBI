import { useEffect, useRef } from "react";
import { axisBottom, axisLeft, max, scaleLinear, scaleSqrt, select } from "d3";
import { cn } from "@/lib/utils";
import { paddedNumericDomain, resolveChartBox } from "@/shared/charts/core/domain";
import {
  joinCartesianLayers,
  styleChartAxis,
  updateCartesianLabels,
  updateHorizontalGrid,
  updateReferenceLines,
} from "@/shared/charts/core/layers";
import { DEFAULT_CARTESIAN_MARGIN } from "@/shared/charts/core/margins";
import { useChartTheme } from "@/shared/charts/core/theme";
import type { ChartMargin, ChartSurfaceVariant } from "@/shared/charts/core/types";
import { useChartContainerSize } from "@/shared/charts/core/useChartContainerSize";
import type { AxisModel, ReferenceLineModel, XYPoint } from "@/shared/charts/ChartModel";
import { plotAxisTickFormatter } from "./axisFormat";

export interface ScatterChartProps {
  data: XYPoint[];
  xAxis: AxisModel;
  yAxis: AxisModel;
  color?: string;
  radius?: number;
  height?: number;
  margin?: ChartMargin;
  symmetricY?: boolean;
  zeroLine?: boolean;
  highlightIndices?: ReadonlySet<number>;
  highlightColor?: string;
  surface?: ChartSurfaceVariant;
  className?: string;
  referenceLines?: ReferenceLineModel[];
  pointSizes?: number[];
  xDomain?: [number, number];
  yDomain?: [number, number];
}

function symmetricDomain(data: XYPoint[]): [number, number] {
  const maximum = data.reduce(
    (current, point) => (Number.isFinite(point.y) ? Math.max(current, Math.abs(point.y)) : current),
    0,
  );
  const paddedMaximum = maximum * 1.15 || 1;
  return [-paddedMaximum, paddedMaximum];
}

export function ScatterChart({
  data,
  xAxis,
  yAxis,
  color,
  radius = 3,
  height: heightProp,
  margin = DEFAULT_CARTESIAN_MARGIN,
  symmetricY = false,
  zeroLine = false,
  highlightIndices,
  highlightColor,
  surface = "card",
  className,
  referenceLines,
  pointSizes,
  xDomain: fixedXDomain,
  yDomain: fixedYDomain,
}: ScatterChartProps) {
  const svgRef = useRef<SVGSVGElement>(null);
  const { containerRef, size } = useChartContainerSize();
  const { colors: chartTheme, series: seriesColors } = useChartTheme();
  const plotColor = color ?? seriesColors.primary;
  const plotHighlightColor = highlightColor ?? seriesColors.highlight;
  const xLabel = xAxis.label;
  const yLabel = yAxis.label;
  const xValueType = xAxis.valueType;
  const yValueType = yAxis.valueType;

  useEffect(() => {
    const svgNode = svgRef.current;
    if (!svgNode) return;

    const svg = select(svgNode);
    const layers = joinCartesianLayers(svg);
    const width = size.width;
    const height = heightProp ?? size.height;
    const box = resolveChartBox(width, height, margin);

    svg
      .attr("width", width)
      .attr("height", height)
      .attr("role", "img")
      .attr("aria-label", `${yLabel ?? "y"} by ${xLabel ?? "x"}`);

    if (data.length === 0 || !box) {
      svg.attr("data-chart-x-domain", null).attr("data-chart-y-domain", null);
      layers.root.attr("display", "none");
      layers.marks
        .selectAll<SVGLineElement, number>('line[data-chart-reference="zero"]')
        .data([])
        .join("line");
      layers.marks
        .selectAll<SVGCircleElement, XYPoint>('circle[data-chart-mark="scatter-point"]')
        .data([])
        .join("circle");
      return;
    }

    layers.root.attr("display", null).attr("transform", `translate(${margin.left},${margin.top})`);

    const references = referenceLines ?? [];
    const xDomain =
      fixedXDomain ??
      paddedNumericDomain(
        [
          ...data.map((point) => point.x),
          ...references.flatMap((value) => [value.start.x, value.end.x]),
        ],
        0.06,
        1,
      );
    const yDomain =
      fixedYDomain ??
      (symmetricY
        ? symmetricDomain(data)
        : paddedNumericDomain(
            [
              ...data.map((point) => point.y),
              ...references.flatMap((value) => [value.start.y, value.end.y]),
            ],
            0.06,
            1,
          ));
    const xScale = scaleLinear().domain(xDomain).range([0, box.plotWidth]);
    const yScale = scaleLinear().domain(yDomain).range([box.plotHeight, 0]);

    svg
      .attr("data-chart-x-domain", JSON.stringify(xDomain))
      .attr("data-chart-y-domain", JSON.stringify(yDomain));

    updateHorizontalGrid(
      layers.grid,
      yScale.ticks(5),
      (value) => yScale(value),
      box.plotWidth,
      chartTheme.grid,
    );

    const xAxisGenerator = axisBottom(xScale).ticks(6).tickSize(-4);
    const xTickFormat = plotAxisTickFormatter(xValueType);
    if (xTickFormat) xAxisGenerator.tickFormat(xTickFormat);
    layers.xAxis.attr("transform", `translate(0,${box.plotHeight})`).call(xAxisGenerator);
    styleChartAxis(layers.xAxis, chartTheme);

    const yAxisGenerator = axisLeft(yScale).ticks(5).tickSize(-4);
    const yTickFormat = plotAxisTickFormatter(yValueType);
    if (yTickFormat) yAxisGenerator.tickFormat(yTickFormat);
    layers.yAxis.call(yAxisGenerator);
    styleChartAxis(layers.yAxis, chartTheme);
    updateCartesianLabels(layers.labels, box, { x: xLabel, y: yLabel }, chartTheme.label);
    updateReferenceLines(
      layers.marks,
      references,
      (value) => xScale(value),
      (value) => yScale(value),
      chartTheme.zeroLine,
    );
    const largestSize = max(pointSizes ?? []) ?? 0;
    const sizeRadius = pointSizes
      ? scaleSqrt()
          .domain([0, largestSize || 1])
          .range([0, 16])
      : null;

    layers.marks
      .selectAll<SVGLineElement, number>('line[data-chart-reference="zero"]')
      .data(zeroLine ? [0] : [])
      .join("line")
      .attr("data-chart-reference", "zero")
      .attr("x1", 0)
      .attr("x2", box.plotWidth)
      .attr("y1", (value) => yScale(value))
      .attr("y2", (value) => yScale(value))
      .attr("stroke", chartTheme.zeroLine)
      .attr("stroke-width", 1);

    layers.marks
      .selectAll<SVGCircleElement, XYPoint>('circle[data-chart-mark="scatter-point"]')
      .data(data)
      .join("circle")
      .attr("data-chart-mark", "scatter-point")
      .attr("data-highlighted", (_, index) => (highlightIndices?.has(index) ? "true" : "false"))
      .attr("cx", (point) => xScale(point.x))
      .attr("cy", (point) => yScale(point.y))
      .attr("r", (_, index) => (sizeRadius ? sizeRadius(pointSizes?.[index] ?? 0) : radius))
      .attr("fill", (_, index) => (highlightIndices?.has(index) ? plotHighlightColor : plotColor))
      .attr("fill-opacity", 0.7)
      .attr("stroke", (_, index) => (highlightIndices?.has(index) ? plotHighlightColor : plotColor))
      .attr("stroke-opacity", 0.3)
      .attr("stroke-width", 1);
  }, [
    chartTheme,
    data,
    referenceLines,
    pointSizes,
    fixedXDomain,
    fixedYDomain,
    heightProp,
    highlightIndices,
    margin,
    plotColor,
    plotHighlightColor,
    radius,
    size,
    symmetricY,
    xLabel,
    xValueType,
    yLabel,
    yValueType,
    zeroLine,
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
    </div>
  );
}
