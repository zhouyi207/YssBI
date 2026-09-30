import { select, type BaseType, type Selection } from "d3";
import type { ChartThemeColors } from "@/shared/theme/chartTheme";
import type { ChartBox } from "./domain";
import type { ReferenceLineModel } from "../ChartModel";

type ChartGroup = Selection<SVGGElement, unknown, null, undefined>;
type ChartSvg = Selection<SVGSVGElement, unknown, null, undefined>;
type LayerName = "root" | "grid" | "x-axis" | "y-axis" | "marks" | "labels";

function joinNamedLayer<Parent extends SVGSVGElement | SVGGElement>(
  parent: Selection<Parent, unknown, null, undefined>,
  name: LayerName,
): ChartGroup {
  const node = parent
    .selectAll<SVGGElement, null>(`:scope > g[data-chart-layer="${name}"]`)
    .data([null])
    .join("g")
    .attr("data-chart-layer", name)
    .node();

  return select<SVGGElement, unknown>(node as SVGGElement);
}

export function joinCartesianLayers(svg: ChartSvg) {
  const root = joinNamedLayer(svg, "root");

  return {
    root,
    grid: joinNamedLayer(root, "grid"),
    xAxis: joinNamedLayer(root, "x-axis"),
    yAxis: joinNamedLayer(root, "y-axis"),
    marks: joinNamedLayer(root, "marks"),
    labels: joinNamedLayer(root, "labels"),
  };
}

export function updateHorizontalGrid(
  layer: ChartGroup,
  ticks: readonly number[],
  yPosition: (value: number) => number,
  plotWidth: number,
  color: string,
): void {
  layer
    .selectAll<SVGLineElement, number>("line")
    .data(ticks)
    .join("line")
    .attr("x1", 0)
    .attr("x2", plotWidth)
    .attr("y1", yPosition)
    .attr("y2", yPosition)
    .attr("stroke", color)
    .attr("stroke-dasharray", "2,3");
}

export function styleChartAxis<Datum, Parent extends BaseType, ParentDatum>(
  layer: Selection<SVGGElement, Datum, Parent, ParentDatum>,
  colors: Pick<ChartThemeColors, "axis" | "tick">,
): void {
  layer.select(".domain").attr("stroke", colors.axis);
  layer.selectAll(".tick line").attr("stroke", colors.axis);
  layer.selectAll(".tick text").attr("fill", colors.tick).attr("font-size", "10px");
}

export function updateReferenceLines(
  layer: ChartGroup,
  lines: readonly ReferenceLineModel[],
  x: (value: number) => number,
  y: (value: number) => number,
  color: string,
): void {
  layer
    .selectAll<SVGLineElement, ReferenceLineModel>('line[data-chart-reference="annotation"]')
    .data(lines)
    .join("line")
    .attr("data-chart-reference", "annotation")
    .attr("x1", (value) => x(value.start.x))
    .attr("x2", (value) => x(value.end.x))
    .attr("y1", (value) => y(value.start.y))
    .attr("y2", (value) => y(value.end.y))
    .attr("stroke", color)
    .attr("stroke-width", 1)
    .attr("stroke-dasharray", "5,4");
}

export function updateCartesianLabels(
  layer: ChartGroup,
  box: ChartBox,
  labels: { x?: string; y?: string },
  color: string,
): void {
  layer
    .selectAll<SVGTextElement, string>('text[data-chart-label="x"]')
    .data(labels.x ? [labels.x] : [])
    .join("text")
    .attr("data-chart-label", "x")
    .attr("x", box.plotWidth / 2)
    .attr("y", box.plotHeight + 32)
    .attr("text-anchor", "middle")
    .attr("fill", color)
    .attr("font-size", "11px")
    .text((label) => label);

  layer
    .selectAll<SVGTextElement, string>('text[data-chart-label="y"]')
    .data(labels.y ? [labels.y] : [])
    .join("text")
    .attr("data-chart-label", "y")
    .attr("transform", "rotate(-90)")
    .attr("x", -box.plotHeight / 2)
    .attr("y", -42)
    .attr("text-anchor", "middle")
    .attr("fill", color)
    .attr("font-size", "11px")
    .text((label) => label);
}
