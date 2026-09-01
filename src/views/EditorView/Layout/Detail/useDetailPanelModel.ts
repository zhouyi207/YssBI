import { useMemo } from "react";
import { useEditorSessionResources } from "@/features/application/editor";
import { useEditorUi } from "@/features/core/editor/ui";
import { useLogStore } from "@/features/application/log";
import { useChartRead } from "@/features/core/chart/read";
import type { ChartDocument } from "@/shared/types/domain/chart";
import { resolveDetailPanelModel } from "./resolveDetailPanelModel";
import type { DetailPanelModel } from "./resolveDetailPanelModel";

export function useDetailPanelModel(): {
  model: DetailPanelModel;
  chartPath: string | null;
  chartName: string | null;
  chartDocument: ChartDocument | null;
} {
  const { variables, events, functions, dataframes } = useEditorSessionResources();
  const target = useEditorUi((snapshot) => snapshot.detailFocus);
  const selectedLog = useLogStore((s) => s.selectedLog);

  const chartPath = target?.kind === "chart" ? target.chartPath : null;

  const chartDocument = useChartRead((snapshot) =>
    chartPath
      ? snapshot.documents[chartPath]
        ? (structuredClone(snapshot.documents[chartPath]) as ChartDocument)
        : null
      : null,
  );
  const chartName = useChartRead((snapshot) =>
    chartPath
      ? (snapshot.index.find((chart) => chart.chartPath === chartPath)?.name ?? null)
      : null,
  );

  const model = useMemo(
    () =>
      resolveDetailPanelModel({
        target,
        selectedLog,
        variables,
        events,
        functions,
        dataframes,
        chartDocument,
      }),
    [target, selectedLog, variables, events, functions, dataframes, chartDocument],
  );

  return { model, chartPath, chartName, chartDocument };
}
