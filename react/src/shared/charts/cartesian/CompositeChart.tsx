import { useEffect, useRef } from "react";
import { axisBottom, axisLeft, axisRight, format, line, scaleBand, scaleLinear, select } from "d3";
import { useTranslation } from "react-i18next";
import { cn } from "@/lib/utils";
import type { ChartModel } from "../ChartModel";
import { paddedNumericDomain, resolveChartBox } from "../core/domain";
import { joinCartesianLayers, styleChartAxis, updateHorizontalGrid } from "../core/layers";
import { useChartTheme } from "../core/theme";
import type { ChartSurfaceVariant } from "../core/types";
import { useChartContainerSize } from "../core/useChartContainerSize";

type CompositeModel = Extract<ChartModel, { kind: "pareto" | "combination" }>;
export function CompositeChart({
  model,
  surface,
}: {
  model: CompositeModel;
  surface: ChartSurfaceVariant;
}) {
  const svgRef = useRef<SVGSVGElement>(null);
  const { containerRef, size } = useChartContainerSize();
  const { colors, series } = useChartTheme();
  const { t } = useTranslation();
  useEffect(() => {
    if (!svgRef.current) return;
    const svg = select(svgRef.current);
    const layers = joinCartesianLayers(svg);
    const margin = { top: 30, right: 56, bottom: 62, left: 60 };
    const box = resolveChartBox(size.width, size.height, margin);
    svg
      .attr("width", size.width)
      .attr("height", size.height)
      .attr("role", "img")
      .attr("aria-label", t(model.kind === "pareto" ? "plot.pareto" : "plot.combination"));
    if (!box) {
      layers.root.attr("display", "none");
      return;
    }
    const labels = model.kind === "pareto" ? model.data.map((value) => value.label) : model.labels;
    const bars = model.kind === "pareto" ? model.data.map((value) => value.count) : model.bars;
    const values =
      model.kind === "pareto" ? model.data.map((value) => value.cumulative) : model.line;
    const dualAxis = model.kind === "pareto" || model.dualAxis;
    layers.root.attr("display", null).attr("transform", `translate(${margin.left},${margin.top})`);
    const keys = labels.map((_, i) => String(i));
    const x = scaleBand().domain(keys).range([0, box.plotWidth]).padding(0.15);
    const y = scaleLinear()
      .domain(paddedNumericDomain([0, ...bars, ...(!dualAxis ? values : [])], 0.06, 1))
      .nice()
      .range([box.plotHeight, 0]);
    const right = dualAxis
      ? scaleLinear()
          .domain(model.kind === "pareto" ? [0, 1] : paddedNumericDomain(values, 0.06, 1))
          .range([box.plotHeight, 0])
      : y;
    const step = Math.max(1, Math.ceil(labels.length / Math.max(1, box.plotWidth / 65)));
    layers.xAxis.attr("transform", `translate(0,${box.plotHeight})`).call(
      axisBottom(x)
        .tickValues(keys.filter((_, i) => i % step === 0))
        .tickFormat((key) => labels[Number(key)].slice(0, 16)),
    );
    layers.yAxis.call(axisLeft(y).ticks(5));
    styleChartAxis(layers.xAxis, colors);
    styleChartAxis(layers.yAxis, colors);
    const rightAxis = layers.root
      .selectAll<SVGGElement, number>("g.right-axis")
      .data(dualAxis ? [0] : [])
      .join("g")
      .attr("class", "right-axis")
      .attr("transform", `translate(${box.plotWidth},0)`);
    const generator = axisRight(right).ticks(5);
    if (model.kind === "pareto") generator.tickFormat(format(".0%"));
    rightAxis.call(generator);
    styleChartAxis(rightAxis, colors);
    updateHorizontalGrid(layers.grid, y.ticks(5), (value) => y(value), box.plotWidth, colors.grid);
    const barMarks = layers.marks
      .selectAll<SVGRectElement, number>("rect.composite-bar")
      .data(bars)
      .join("rect")
      .attr("class", "composite-bar")
      .attr("data-chart-mark", "composite-bar")
      .attr("x", (_, i) => x(String(i)) ?? 0)
      .attr("width", x.bandwidth())
      .attr("y", (value) => y(Math.max(0, value)))
      .attr("height", (value) => Math.abs(y(value) - y(0)))
      .attr("fill", series.primary)
      .attr("fill-opacity", 0.65);
    barMarks.each(function (value, index) {
      select(this)
        .selectAll<SVGTitleElement, number>("title")
        .data([value])
        .join("title")
        .text(`${labels[index]}: ${format(".5~g")(value)}`);
    });
    const data = values.map((value, i) => ({
      x: (x(String(i)) ?? 0) + x.bandwidth() / 2,
      y: right(value),
    }));
    const path = line<{ x: number; y: number }>()
      .x((point) => point.x)
      .y((point) => point.y);
    layers.marks
      .selectAll<SVGPathElement, typeof data>("path.composite-line")
      .data([data])
      .join("path")
      .attr("class", "composite-line")
      .attr("data-chart-mark", "composite-line")
      .attr("d", (points) => path(points))
      .attr("fill", "none")
      .attr("stroke", series.secondary)
      .attr("stroke-width", 2);
    const pointMarks = layers.marks
      .selectAll<SVGCircleElement, number>("circle.composite-point")
      .data(values)
      .join("circle")
      .attr("class", "composite-point")
      .attr("data-chart-mark", "composite-point")
      .attr("cx", (_, i) => data[i].x)
      .attr("cy", (_, i) => data[i].y)
      .attr("r", 3)
      .attr("fill", series.secondary)
      .attr("data-index", (_, i) => i);
    pointMarks
      .selectAll<SVGTitleElement, number>("title")
      .data((value) => [value])
      .join("title")
      .text((value) => (model.kind === "pareto" ? format(".1%")(value) : format(".5~g")(value)));
    const legends = [
      t(model.kind === "pareto" ? "plot.frequency" : "plot.bars"),
      t(model.kind === "pareto" ? "plot.cumulative" : "plot.line"),
    ];
    layers.labels
      .selectAll<SVGTextElement, string>("text.legend")
      .data(legends)
      .join("text")
      .attr("class", "legend")
      .attr("x", (_, i) => i * 130)
      .attr("y", -10)
      .attr("fill", (_, i) => (i === 0 ? series.primary : series.secondary))
      .attr("font-size", 11)
      .text((value) => value);
  }, [colors, model, series, size, t]);
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
