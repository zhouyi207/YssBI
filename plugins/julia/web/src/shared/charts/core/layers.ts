import { select, type Selection } from "d3";
import type { ChartThemeColors } from "@/shared/theme/chartTheme";
import type { ChartBox } from "./domain";

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

export function styleChartAxis(
  layer: ChartGroup,
  colors: Pick<ChartThemeColors, "axis" | "tick">,
): void {
  layer.select(".domain").attr("stroke", colors.axis);
  layer.selectAll(".tick line").attr("stroke", colors.axis);
  layer.selectAll(".tick text").attr("fill", colors.tick).attr("font-size", "10px");
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
