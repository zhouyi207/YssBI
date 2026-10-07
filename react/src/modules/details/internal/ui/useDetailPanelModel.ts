import { useMemo } from "react";
import { useShallow } from "zustand/react/shallow";
import { useEditorUi } from "@/features/core/editor/ui";
import { useLogStore } from "@/features/application/log";
import { useChartRead } from "@/features/core/chart/read";
import { useDatabaseRead } from "@/features/core/database/read";
import { useResourceRead } from "@/features/core/resource/read";
import { useResourceStore } from "@/features/core/resource/resourceStore";
import { buildFunctionResourceView } from "@/features/core/resource/functionResourceView";
import { lookupNodeFileKind } from "@/features/core/resource/resourceSelectors";
import { useGraphRead } from "@/features/core/graph/read";
import { resourceKey } from "@/features/core/resource/resourceTypes";
import { resolveDetailPanelModel } from "./resolveDetailPanelModel";
import type { DetailPanelModel } from "./resolveDetailPanelModel";

export function useDetailPanelModel(): DetailPanelModel {
  const focus = useEditorUi((snapshot) => snapshot.detailFocus);
  const nodeExists = useGraphRead(
    (snapshot) =>
      focus?.kind !== "node" || Boolean(snapshot.graphEntities[focus.graphPath]?.nodes[focus.id]),
  );
  const nodeGraphKind = useResourceRead((snapshot) =>
    focus?.kind === "node" && !nodeExists
      ? lookupNodeFileKind(snapshot.resources, focus.graphPath)
      : undefined,
  );
  const target = useMemo(
    () =>
      focus?.kind === "node" && !nodeExists
        ? nodeGraphKind
          ? { kind: nodeGraphKind, path: focus.graphPath }
          : null
        : focus,
    [focus, nodeExists, nodeGraphKind],
  );
  const selectedLog = useLogStore((s) => (target?.kind === "log" ? s.selectedLog : null));
  const graphName = useResourceRead((snapshot) => {
    if (target?.kind !== "event_graph" && target?.kind !== "function_graph") return null;
    const resource = snapshot.resources[resourceKey({ kind: target.kind, id: target.path })];
    return resource?.exists ? resource.name : null;
  });
  const functionSignature = useResourceStore(
    useShallow((snapshot) => {
      const meta =
        target?.kind === "function_graph" && graphName !== null
          ? snapshot.graphMeta[target.path]
          : undefined;
      if (!meta) return undefined;
      return { functionInputs: meta.functionInputs, functionOutputs: meta.functionOutputs };
    }),
  );
  const dataframe = useDatabaseRead((snapshot) =>
    target?.kind === "data" ? (snapshot.databases[target.id] ?? null) : null,
  );

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
        eventName: target?.kind === "event_graph" ? graphName : null,
        functionGraph:
          target?.kind === "function_graph" && graphName !== null
            ? buildFunctionResourceView({ id: target.path, name: graphName }, functionSignature)
            : null,
        dataframe,
        chartDocument,
        chartName,
      }),
    [target, selectedLog, graphName, functionSignature, dataframe, chartDocument, chartName],
  );
}
