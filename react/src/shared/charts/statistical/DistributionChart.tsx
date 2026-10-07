import { useEffect, useRef } from "react";
import { area, axisBottom, axisLeft, format, max, scaleBand, scaleLinear, select } from "d3";
import { useTranslation } from "react-i18next";
import { cn } from "@/lib/utils";
import type { DistributionGroupPlotDTO, PlotPointDTO } from "@/shared/types/domain/plotPayload";
import { paddedNumericDomain, resolveChartBox } from "../core/domain";
import { joinCartesianLayers, styleChartAxis, updateHorizontalGrid } from "../core/layers";
import { useChartTheme } from "../core/theme";
import type { ChartSurfaceVariant } from "../core/types";
import { useChartContainerSize } from "../core/useChartContainerSize";

export function DistributionChart({
  groups,
  violin,
  surface,
}: {
  groups: DistributionGroupPlotDTO[];
  violin: boolean;
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
    const margin = { top: 24, right: 24, bottom: 64, left: 60 };
    const box = resolveChartBox(size.width, size.height, margin);
    svg
      .attr("width", size.width)
      .attr("height", size.height)
      .attr("role", "img")
      .attr("aria-label", t(violin ? "plot.violin" : "plot.boxplot"));
    if (!box || !groups.length) {
      layers.root.attr("display", "none");
      return;
    }
    layers.root.attr("display", null).attr("transform", `translate(${margin.left},${margin.top})`);
    const indices = groups.map((_, i) => String(i));
    const x = scaleBand().domain(indices).range([0, box.plotWidth]).padding(0.3);
    const values = groups.flatMap((group) => [
      group.lowerWhisker,
      group.upperWhisker,
      ...group.outliers,
      ...(violin ? group.density.map((point) => point.x) : []),
    ]);
    const y = scaleLinear()
      .domain(paddedNumericDomain(values, 0.05, 1))
      .nice()
      .range([box.plotHeight, 0]);
    updateHorizontalGrid(layers.grid, y.ticks(5), (value) => y(value), box.plotWidth, colors.grid);
    const step = Math.max(1, Math.ceil(groups.length / Math.max(1, box.plotWidth / 65)));
    layers.xAxis.attr("transform", `translate(0,${box.plotHeight})`).call(
      axisBottom(x)
        .tickValues(indices.filter((_, i) => i % step === 0))
        .tickFormat((key) => groups[Number(key)].label.slice(0, 16)),
    );
    layers.yAxis.call(axisLeft(y).ticks(5));
    styleChartAxis(layers.xAxis, colors);
    styleChartAxis(layers.yAxis, colors);
    const marks = layers.marks
      .selectAll<SVGGElement, DistributionGroupPlotDTO>("g.distribution-group")
      .data(groups)
      .join("g")
      .attr("class", "distribution-group")
      .attr("transform", (_, i) => `translate(${(x(String(i)) ?? 0) + x.bandwidth() / 2},0)`);
    marks.each(function (group, index) {
      const mark = select(this);
      const color = series.palette[index % series.palette.length];
      const width = x.bandwidth() * (violin ? 0.18 : 0.7);
      const densityMax = max(group.density, (point) => point.y) ?? 1;
      const outline = area<PlotPointDTO>()
        .y((point) => y(point.x))
        .x0((point) => (-x.bandwidth() * 0.5 * point.y) / (densityMax || 1))
        .x1((point) => (x.bandwidth() * 0.5 * point.y) / (densityMax || 1));
      mark
        .selectAll<SVGPathElement, PlotPointDTO[]>("path.violin-density")
        .data(violin ? [group.density] : [])
        .join("path")
        .attr("class", "violin-density")
        .attr("data-chart-mark", "violin-density")
        .attr("d", (points) => outline(points))
        .attr("fill", color)
        .attr("fill-opacity", 0.25)
        .attr("stroke", color);
      mark
        .selectAll<SVGLineElement, DistributionGroupPlotDTO>("line.whisker")
        .data([group])
        .join("line")
        .attr("class", "whisker")
        .attr("data-chart-mark", "box-whisker")
        .attr("x1", 0)
        .attr("x2", 0)
        .attr("y1", (value) => y(value.lowerWhisker))
        .attr("y2", (value) => y(value.upperWhisker))
        .attr("stroke", color);
      mark
        .selectAll<SVGRectElement, DistributionGroupPlotDTO>("rect.box")
        .data([group])
        .join("rect")
        .attr("class", "box")
        .attr("data-chart-mark", "box-quartiles")
        .attr("x", -width / 2)
        .attr("width", width)
        .attr("y", (value) => y(value.q3))
        .attr("height", (value) => Math.max(1, y(value.q1) - y(value.q3)))
        .attr("fill", color)
        .attr("fill-opacity", violin ? 0.6 : 0.25)
        .attr("stroke", color);
      mark
        .selectAll<SVGLineElement, number>("line.cap")
        .data([group.lowerWhisker, group.median, group.upperWhisker])
        .join("line")
        .attr("class", "cap")
        .attr("x1", -width / 2)
        .attr("x2", width / 2)
        .attr("y1", (value) => y(value))
        .attr("y2", (value) => y(value))
        .attr("stroke", color)
        .attr("stroke-width", (_, i) => (i === 1 ? 2 : 1));
      mark
        .selectAll<SVGCircleElement, number>("circle.outlier")
        .data(violin ? [] : group.outliers)
        .join("circle")
        .attr("class", "outlier")
        .attr("data-chart-mark", "box-outlier")
        .attr("cx", 0)
        .attr("cy", (value) => y(value))
        .attr("r", 2.5)
        .attr("fill", "none")
        .attr("stroke", color);
      mark
        .selectAll<SVGTitleElement, DistributionGroupPlotDTO>("title")
        .data([group])
        .join("title")
        .text(
          (value) =>
            `${value.label}\nN = ${value.observations}\nQ1 = ${format(".5~g")(value.q1)}\n${t("plot.median")} = ${format(".5~g")(value.median)}\nQ3 = ${format(".5~g")(value.q3)}\n${t("plot.outliers")} = ${value.outlierCount}`,
        );
    });
  }, [colors, groups, series, size, t, violin]);
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
