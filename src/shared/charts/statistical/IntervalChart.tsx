import { useEffect, useRef } from "react";
import { axisBottom, axisLeft, format, scaleBand, scaleLinear, select } from "d3";
import { useTranslation } from "react-i18next";
import { cn } from "@/lib/utils";
import type { ChartModel } from "../ChartModel";
import { paddedNumericDomain, resolveChartBox } from "../core/domain";
import { joinCartesianLayers, styleChartAxis, updateHorizontalGrid } from "../core/layers";
import { useChartTheme } from "../core/theme";
import type { ChartSurfaceVariant } from "../core/types";
import { useChartContainerSize } from "../core/useChartContainerSize";

type IntervalModel = Extract<ChartModel, { kind: "errorbar" | "coefficient" }>;
export function IntervalChart({
  model,
  surface,
}: {
  model: IntervalModel;
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
    const horizontal = model.kind === "coefficient";
    const margin = {
      top: 20,
      right: 24,
      bottom: 42,
      left: horizontal ? Math.min(140, size.width * 0.3) : 60,
    };
    const box = resolveChartBox(size.width, size.height, margin);
    svg
      .attr("width", size.width)
      .attr("height", size.height)
      .attr("role", "img")
      .attr("aria-label", t(horizontal ? "plot.coefficient" : "plot.errorbar"));
    if (!box || !model.data.length) {
      layers.root.attr("display", "none");
      return;
    }
    layers.root.attr("display", null).attr("transform", `translate(${margin.left},${margin.top})`);
    const rows =
      model.kind === "coefficient"
        ? model.data.map((point, i) => ({
            x: point.value,
            y: i,
            lower: point.lower,
            upper: point.upper,
            label: point.label,
          }))
        : model.data.map((point) => ({ ...point, label: `${format(".5~g")(point.x)}` }));
    const values = rows.flatMap((row) => [row.lower, row.upper]);
    const x = scaleLinear()
      .domain(paddedNumericDomain(horizontal ? [0, ...values] : rows.map((row) => row.x), 0.06, 1))
      .range([0, box.plotWidth]);
    const y = scaleLinear()
      .domain(paddedNumericDomain(horizontal ? rows.map((row) => row.y) : values, 0.06, 1))
      .range([box.plotHeight, 0]);
    const bands = scaleBand()
      .domain(rows.map((_, i) => String(i)))
      .range([0, box.plotHeight])
      .padding(0.3);
    const yPosition = (index: number, value: number) =>
      horizontal ? (bands(String(index)) ?? 0) + bands.bandwidth() / 2 : y(value);
    layers.xAxis.attr("transform", `translate(0,${box.plotHeight})`).call(axisBottom(x).ticks(6));
    if (horizontal) {
      const step = Math.max(1, Math.ceil(rows.length / Math.max(1, box.plotHeight / 20)));
      layers.yAxis.call(
        axisLeft(bands)
          .tickValues(bands.domain().filter((_, i) => i % step === 0))
          .tickFormat((key) => rows[Number(key)].label.slice(0, 22)),
      );
      updateHorizontalGrid(layers.grid, [], () => 0, box.plotWidth, colors.grid);
    } else {
      layers.yAxis.call(axisLeft(y).ticks(5));
      updateHorizontalGrid(
        layers.grid,
        y.ticks(5),
        (value) => y(value),
        box.plotWidth,
        colors.grid,
      );
    }
    styleChartAxis(layers.xAxis, colors);
    styleChartAxis(layers.yAxis, colors);
    layers.marks
      .selectAll<SVGLineElement, number>("line.zero")
      .data(horizontal ? [0] : [])
      .join("line")
      .attr("class", "zero")
      .attr("x1", (value) => x(value))
      .attr("x2", (value) => x(value))
      .attr("y1", 0)
      .attr("y2", box.plotHeight)
      .attr("stroke", colors.zeroLine)
      .attr("stroke-dasharray", "4,4");
    const marks = layers.marks
      .selectAll<SVGGElement, (typeof rows)[number]>("g.interval")
      .data(rows)
      .join("g")
      .attr("class", "interval");
    marks.each(function (row, i) {
      const mark = select(this);
      const cy = yPosition(i, row.y);
      const cx = x(row.x);
      mark
        .selectAll<SVGLineElement, number>("line.range")
        .data([0])
        .join("line")
        .attr("class", "range")
        .attr("data-chart-mark", "interval-range")
        .attr("x1", horizontal ? x(row.lower) : cx)
        .attr("x2", horizontal ? x(row.upper) : cx)
        .attr("y1", horizontal ? cy : y(row.lower))
        .attr("y2", horizontal ? cy : y(row.upper))
        .attr("stroke", series.primary)
        .attr("stroke-width", 1.5);
      mark
        .selectAll<SVGLineElement, number>("line.cap")
        .data([row.lower, row.upper])
        .join("line")
        .attr("class", "cap")
        .attr("x1", (value) => (horizontal ? x(value) : cx - 5))
        .attr("x2", (value) => (horizontal ? x(value) : cx + 5))
        .attr("y1", (value) => (horizontal ? cy - 5 : y(value)))
        .attr("y2", (value) => (horizontal ? cy + 5 : y(value)))
        .attr("stroke", series.primary);
      mark
        .selectAll<SVGCircleElement, number>("circle.estimate")
        .data([0])
        .join("circle")
        .attr("class", "estimate")
        .attr("data-chart-mark", "interval-estimate")
        .attr("cx", cx)
        .attr("cy", cy)
        .attr("r", 3.5)
        .attr("fill", series.primary);
      mark
        .selectAll<SVGTitleElement, number>("title")
        .data([0])
        .join("title")
        .text(
          `${row.label}: ${format(".5~g")(horizontal ? row.x : row.y)}\n[${format(".5~g")(row.lower)}, ${format(".5~g")(row.upper)}]`,
        );
    });
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
