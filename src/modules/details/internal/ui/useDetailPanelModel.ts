import { useMemo } from "react";
import { useEditorCollections } from "@/features/core/editor";
import { useEditorUi } from "@/features/core/editor/ui";
import { useLogStore } from "@/features/application/log";
import { useChartRead } from "@/features/core/chart/read";
import { useResourceRead } from "@/features/core/resource/read";
import { resourceKey } from "@/features/core/resource/resourceTypes";
import { resolveDetailPanelModel } from "./resolveDetailPanelModel";
import type { DetailPanelModel } from "./resolveDetailPanelModel";

export function useDetailPanelModel(): DetailPanelModel {
  const { eventGraphs, functionGraphs, dataframes } = useEditorCollections();
  const target = useEditorUi((snapshot) => snapshot.detailFocus);
  const selectedLog = useLogStore((s) => s.selectedLog);

  const chartPath = target?.kind === "chart" ? target.chartPath : null;

  const chartDocument = useChartRead((snapshot) =>
    chartPath ? (snapshot.documents[chartPath] ?? null) : null,
  );
  const chartName = useResourceRead((snapshot) =>
    chartPath
      ? (snapshot.resources[resourceKey({ kind: "chart", id: chartPath })]?.name ?? null)
      : null,
  );

  return useMemo(
    () =>
      resolveDetailPanelModel({
        target,
        selectedLog,
        eventGraphs,
        functionGraphs,
        dataframes,
        chartDocument,
        chartName,
      }),
    [target, selectedLog, eventGraphs, functionGraphs, dataframes, chartDocument, chartName],
  );
}
