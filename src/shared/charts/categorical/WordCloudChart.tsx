import { useEffect, useRef } from "react";
import { max, scaleSqrt, select } from "d3";
import { useTranslation } from "react-i18next";
import { cn } from "@/lib/utils";
import type { WordCountPlotDTO } from "@/shared/types/domain/plotPayload";
import { useChartTheme } from "../core/theme";
import type { ChartSurfaceVariant } from "../core/types";
import { useChartContainerSize } from "../core/useChartContainerSize";

type PlacedWord = WordCountPlotDTO & {
  x: number;
  y: number;
  fontSize: number;
  width: number;
  height: number;
};
export function WordCloudChart({
  words,
  surface,
}: {
  words: WordCountPlotDTO[];
  surface: ChartSurfaceVariant;
}) {
  const svgRef = useRef<SVGSVGElement>(null);
  const { containerRef, size } = useChartContainerSize();
  const { series } = useChartTheme();
  const { t } = useTranslation();
  useEffect(() => {
    if (!svgRef.current) return;
    const svg = select(svgRef.current);
    svg
      .attr("width", size.width)
      .attr("height", size.height)
      .attr("role", "img")
      .attr("aria-label", t("plot.wordcloud"));
    const context = document.createElement("canvas").getContext("2d");
    if (!context || size.width < 40 || size.height < 40) return;
    const maximum = max(words, (word) => word.count) ?? 1;
    const font = scaleSqrt()
      .domain([0, maximum])
      .range([10, Math.min(70, size.width / 8, size.height / 5)]);
    const placed: PlacedWord[] = [];
    for (const word of words) {
      let fontSize = font(word.count);
      context.font = `600 ${fontSize}px sans-serif`;
      const measured = context.measureText(word.label).width;
      if (measured > size.width - 30) fontSize *= (size.width - 30) / measured;
      context.font = `600 ${fontSize}px sans-serif`;
      const width = context.measureText(word.label).width + 8;
      const height = fontSize * 1.2 + 4;
      for (let attempt = 0; attempt < 1600; attempt++) {
        const angle = attempt * 0.32;
        const radius = Math.sqrt(attempt) * 4.5;
        const x = size.width / 2 + Math.cos(angle) * radius * Math.max(1, size.width / size.height);
        const y = size.height / 2 + Math.sin(angle) * radius;
        if (
          x - width / 2 < 8 ||
          x + width / 2 > size.width - 8 ||
          y - height / 2 < 8 ||
          y + height / 2 > size.height - 8
        )
          continue;
        if (
          placed.some(
            (other) =>
              Math.abs(x - other.x) < (width + other.width) / 2 &&
              Math.abs(y - other.y) < (height + other.height) / 2,
          )
        )
          continue;
        placed.push({ ...word, x, y, fontSize, width, height });
        break;
      }
    }
    const marks = svg
      .selectAll<SVGTextElement, PlacedWord>("text.word")
      .data(placed, (word) => word.label)
      .join("text")
      .attr("class", "word")
      .attr("data-chart-mark", "wordcloud-word")
      .attr("x", (word) => word.x)
      .attr("y", (word) => word.y)
      .attr("text-anchor", "middle")
      .attr("dominant-baseline", "middle")
      .attr("font-family", "sans-serif")
      .attr("font-size", (word) => word.fontSize)
      .attr("font-weight", 600)
      .attr("fill", (_, i) => series.palette[i % series.palette.length])
      .text((word) => word.label);
    marks.append("title").text((word) => `${word.label}: ${word.count}`);
    svg.attr("data-displayed-words", placed.length);
  }, [series, size, t, words]);
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
