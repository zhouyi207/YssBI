import { useEffect, useRef } from "react";
import { axisBottom, axisLeft, curveStepAfter, line, scaleLinear, select } from "d3";
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
import type { ChartMargin, ChartSurfaceVariant } from "@/shared/charts/core/types";
import { useChartContainerSize } from "@/shared/charts/core/useChartContainerSize";
import type { AxisModel, XYPoint } from "@/shared/charts/ChartModel";
import { plotAxisTickFormatter } from "./axisFormat";

export interface EcdfChartProps {
  data: XYPoint[];
  xAxis: AxisModel;
  yAxis: AxisModel;
  color?: string;
  height?: number;
  margin?: ChartMargin;
  surface?: ChartSurfaceVariant;
  className?: string;
}

interface EcdfPathDatum {
  key: "ecdf";
  points: XYPoint[];
}

export function EcdfChart({
  data,
  xAxis,
  yAxis,
  color,
  height: heightProp,
  margin = DEFAULT_CARTESIAN_MARGIN,
  surface = "card",
  className,
}: EcdfChartProps) {
  const svgRef = useRef<SVGSVGElement>(null);
  const { containerRef, size } = useChartContainerSize();
  const { colors: chartTheme, series: seriesColors } = useChartTheme();
  const plotColor = color ?? seriesColors.primary;
  const xLabel = xAxis.label;
  const yLabel = yAxis.label ?? "Cumulative Proportion";
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
      .attr("aria-label", `${yLabel ?? "cumulative proportion"} by ${xLabel ?? "value"}`);

    if (data.length === 0 || !box) {
      svg.attr("data-chart-x-domain", null).attr("data-chart-y-domain", null);
      layers.root.attr("display", "none");
      layers.marks
        .selectAll<SVGPathElement, EcdfPathDatum>('path[data-chart-mark="ecdf-path"]')
        .data([], (datum) => datum.key)
        .join("path");
      return;
    }

    layers.root.attr("display", null).attr("transform", `translate(${margin.left},${margin.top})`);

    const xDomain = paddedNumericDomain(
      data.map((point) => point.x),
      0.06,
      1,
    );
    const yDomain: [number, number] = [0, 1];
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

    const stepPoints: XYPoint[] = [{ x: xDomain[0], y: 0 }, ...data];
    const stepLine = line<XYPoint>()
      .x((point) => xScale(point.x))
      .y((point) => yScale(point.y))
      .curve(curveStepAfter);

    layers.marks
      .selectAll<SVGPathElement, EcdfPathDatum>('path[data-chart-mark="ecdf-path"]')
      .data([{ key: "ecdf", points: stepPoints }], (datum) => datum.key)
      .join("path")
      .attr("data-chart-mark", "ecdf-path")
      .attr("data-chart-curve", "step-after")
      .attr("d", (datum) => stepLine(datum.points))
      .attr("fill", "none")
      .attr("stroke", plotColor)
      .attr("stroke-width", 2)
      .attr("stroke-linecap", "round")
      .attr("stroke-linejoin", "round");
  }, [
    chartTheme,
    data,
    heightProp,
    margin,
    plotColor,
    size,
    xLabel,
    xValueType,
    yLabel,
    yValueType,
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
