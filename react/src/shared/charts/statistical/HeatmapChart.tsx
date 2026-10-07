import { useEffect, useRef } from "react";
import {
  axisBottom,
  axisLeft,
  extent,
  format,
  interpolateRdBu,
  scaleBand,
  scaleDiverging,
  select,
} from "d3";
import { useTranslation } from "react-i18next";
import { cn } from "@/lib/utils";
import type { ChartModel } from "../ChartModel";
import { resolveChartBox } from "../core/domain";
import { joinCartesianLayers, styleChartAxis } from "../core/layers";
import { useChartTheme } from "../core/theme";
import type { ChartSurfaceVariant } from "../core/types";
import { useChartContainerSize } from "../core/useChartContainerSize";

export function HeatmapChart({
  model,
  surface,
}: {
  model: Extract<ChartModel, { kind: "heatmap" }>;
  surface: ChartSurfaceVariant;
}) {
  const svgRef = useRef<SVGSVGElement>(null);
  const { containerRef, size } = useChartContainerSize();
  const { colors } = useChartTheme();
  const { t } = useTranslation();
  useEffect(() => {
    if (!svgRef.current) return;
    const svg = select(svgRef.current);
    const layers = joinCartesianLayers(svg);
    const margin = { top: 32, right: 24, bottom: 62, left: 64 };
    const box = resolveChartBox(size.width, size.height, margin);
    svg
      .attr("width", size.width)
      .attr("height", size.height)
      .attr("role", "img")
      .attr("aria-label", t("plot.heatmap"));
    if (!box || !model.matrix.length) {
      layers.root.attr("display", "none");
      return;
    }
    layers.root.attr("display", null).attr("transform", `translate(${margin.left},${margin.top})`);
    const x = scaleBand()
      .domain(model.xLabels.map((_, i) => String(i)))
      .range([0, box.plotWidth])
      .padding(0.015);
    const y = scaleBand()
      .domain(model.yLabels.map((_, i) => String(i)))
      .range([0, box.plotHeight])
      .padding(0.015);
    const cells = model.matrix.flatMap((row, r) => row.map((value, c) => ({ r, c, value })));
    const [minimum, maximum] = extent(cells, (cell) => cell.value);
    if (minimum === undefined || maximum === undefined) {
      layers.root.attr("display", "none");
      return;
    }
    const midpoint = minimum * 0.5 + maximum * 0.5;
    const color = scaleDiverging(interpolateRdBu).domain([maximum, midpoint, minimum]);
    const xStep = Math.max(1, Math.ceil(model.xLabels.length / Math.max(1, box.plotWidth / 60)));
    const yStep = Math.max(1, Math.ceil(model.yLabels.length / Math.max(1, box.plotHeight / 18)));
    layers.xAxis.attr("transform", `translate(0,${box.plotHeight})`).call(
      axisBottom(x)
        .tickValues(x.domain().filter((_, i) => i % xStep === 0))
        .tickFormat((key) => model.xLabels[Number(key)].slice(0, 16)),
    );
    layers.yAxis.call(
      axisLeft(y)
        .tickValues(y.domain().filter((_, i) => i % yStep === 0))
        .tickFormat((key) => model.yLabels[Number(key)]),
    );
    styleChartAxis(layers.xAxis, colors);
    styleChartAxis(layers.yAxis, colors);
    const marks = layers.marks
      .selectAll<SVGRectElement, (typeof cells)[number]>("rect.heatmap-cell")
      .data(cells, (cell) => `${cell.r}:${cell.c}`)
      .join("rect")
      .attr("class", "heatmap-cell")
      .attr("data-chart-mark", "heatmap-cell")
      .attr("x", (cell) => x(String(cell.c)) ?? 0)
      .attr("y", (cell) => y(String(cell.r)) ?? 0)
      .attr("width", x.bandwidth())
      .attr("height", y.bandwidth())
      .attr("fill", (cell) => color(cell.value));
    marks
      .selectAll<SVGTitleElement, (typeof cells)[number]>("title")
      .data((cell) => [cell])
      .join("title")
      .text(
        (cell) =>
          `${model.yLabels[cell.r]} · ${model.xLabels[cell.c]}: ${format(".6~g")(cell.value)}`,
      );
    layers.labels
      .selectAll<SVGTextElement, number>("text.color-range")
      .data([minimum, maximum])
      .join("text")
      .attr("class", "color-range")
      .attr("x", (_, i) => (i ? box.plotWidth : 0))
      .attr("y", -12)
      .attr("text-anchor", (_, i) => (i ? "end" : "start"))
      .attr("fill", colors.label)
      .attr("font-size", 11)
      .text((value, i) => `${t(i ? "plot.maximum" : "plot.minimum")}: ${format(".5~g")(value)}`);
  }, [colors, model, size, t]);
  return (
    <div
      ref={containerRef}
      className={cn(
        "relative h-full w-full min-h-0 overflow-hidden",
        surface === "card" && "rounded-lg border border-border bg-card",
      )}
    >
      <svg ref={svgRef} />
    </div>
  );
}
