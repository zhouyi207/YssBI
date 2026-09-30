import type React from "react";
import { EcdfChart } from "./cartesian/EcdfChart";
import { HistogramChart } from "./cartesian/HistogramChart";
import { KdeChart } from "./cartesian/KdeChart";
import { LineChart } from "./cartesian/LineChart";
import { ScatterChart } from "./cartesian/ScatterChart";
import { useChartTheme } from "./core/theme";
import type { ChartSurfaceVariant } from "./core/types";
import { CorrelationMatrixChart } from "./statistical/CorrelationMatrixChart";
import { CorrelogramChart } from "./statistical/CorrelogramChart";
import { DistributionChart } from "./statistical/DistributionChart";
import { IntervalChart } from "./statistical/IntervalChart";
import { CompositeChart } from "./cartesian/CompositeChart";
import { HeatmapChart } from "./statistical/HeatmapChart";
import { WordCloudChart } from "./categorical/WordCloudChart";
import type { ChartModel } from "@/shared/charts/ChartModel";

type ChartModelKind = ChartModel["kind"];
type ChartRendererMap = {
  [K in ChartModelKind]: React.ComponentType<{
    model: Extract<ChartModel, { kind: K }>;
    surface: ChartSurfaceVariant;
  }>;
};

const chartRenderers = {
  scatter: ({ model, surface }) => (
    <ScatterChart
      data={model.points}
      xAxis={model.xAxis}
      yAxis={model.yAxis}
      symmetricY={model.symmetricY}
      zeroLine={model.zeroLine}
      highlightIndices={model.highlightIndices ? new Set(model.highlightIndices) : undefined}
      surface={surface}
      referenceLines={model.referenceLines}
      pointSizes={model.pointSizes}
      xDomain={model.xDomain}
      yDomain={model.yDomain}
    />
  ),
  line: ({ model, surface }) => (
    <div
      className={
        surface === "card"
          ? "h-full min-h-0 overflow-hidden rounded-lg border border-border bg-card"
          : "h-full min-h-0 overflow-hidden"
      }
    >
      <LineChart
        data={model.points}
        xAxis={model.xAxis}
        yAxis={model.yAxis}
        showPoints={model.showPoints}
        referenceLines={model.referenceLines}
        xDomain={model.xDomain}
        yDomain={model.yDomain}
      />
    </div>
  ),
  histogram: ({ model, surface }) => (
    <HistogramChart
      data={model.bins}
      xLabel={model.xLabel}
      yLabel={model.yLabel}
      compact={model.compact}
      surface={surface}
    />
  ),
  ecdf: ({ model, surface }) => (
    <EcdfChart data={model.points} xAxis={model.xAxis} yAxis={model.yAxis} surface={surface} />
  ),
  kde: ({ model, surface }) => (
    <KdeChart
      data={model.points}
      xLabel={model.xAxis.label}
      yLabel={model.yAxis.label}
      xMin={model.xMin}
      className={surface === "card" ? "rounded-lg border border-border bg-card" : undefined}
    />
  ),
  correlation: ({ model, surface }) => (
    <CorrelationMatrixChart
      labels={model.labels}
      matrix={model.matrix}
      pMatrix={model.pMatrix}
      surface={surface}
    />
  ),
  correlogram: function CorrelogramRenderer({ model }) {
    const { series } = useChartTheme();

    return (
      <div className="flex min-h-0 w-full flex-1 flex-col gap-2">
        <CorrelogramChart
          data={model.acf}
          ciHalfWidth={model.ciHalfWidth}
          title="ACF"
          valueLabel="ACF"
        />
        <CorrelogramChart
          data={model.pacf}
          ciHalfWidth={model.ciHalfWidth}
          title="PACF"
          color={series.secondary}
          valueLabel="PACF"
        />
      </div>
    );
  },
  boxplot: ({ model, surface }) => (
    <DistributionChart groups={model.groups} violin={false} surface={surface} />
  ),
  violin: ({ model, surface }) => (
    <DistributionChart groups={model.groups} violin surface={surface} />
  ),
  wordcloud: ({ model, surface }) => <WordCloudChart words={model.words} surface={surface} />,
  errorbar: ({ model, surface }) => <IntervalChart model={model} surface={surface} />,
  coefficient: ({ model, surface }) => <IntervalChart model={model} surface={surface} />,
  pareto: ({ model, surface }) => <CompositeChart model={model} surface={surface} />,
  combination: ({ model, surface }) => <CompositeChart model={model} surface={surface} />,
  heatmap: ({ model, surface }) => <HeatmapChart model={model} surface={surface} />,
} satisfies ChartRendererMap;

export interface ChartRendererProps {
  model: ChartModel;
  surface?: ChartSurfaceVariant;
}

export function ChartRenderer({ model, surface = "card" }: ChartRendererProps) {
  const Renderer = chartRenderers[model.kind] as React.ComponentType<{
    model: ChartModel;
    surface: ChartSurfaceVariant;
  }>;

  return <Renderer model={model} surface={surface} />;
}
