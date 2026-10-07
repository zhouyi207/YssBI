import type { DetailTarget } from "@/features/core/editor/detail/detailTypes";
import type { ResourceKind } from "@/shared/types/domain/resource";
import type { FunctionResourceView } from "@/features/core/resource/functionResourceView";
import type { FunctionPinSpec } from "@/shared/types/domain/graph";
import type { ChartDocument } from "@/shared/types/domain/chart";
import type { DatabaseRecord } from "@/shared/types/domain/database";
import type { DeepReadonly } from "@/shared/types/deepReadonly";
import type { LogRecordDto } from "@/shared/types/domain/log";

export interface DetailPanelResolveInput {
  target: DetailTarget | null;
  selectedLog: LogRecordDto | null;
  eventName: string | null;
  functionGraph: FunctionResourceView | null;
  dataframe: DeepReadonly<DatabaseRecord> | null;
  chartDocument: ChartDocument | null;
  chartName: string | null;
}

export type FunctionDetailModel = {
  name: string;
  inputs: FunctionPinSpec[];
  outputs: FunctionPinSpec[];
};

export type DetailPanelModel =
  | { kind: "empty" }
  | { kind: "log"; log: LogRecordDto }
  | { kind: "node"; nodeId: string; graphPath: string }
  | { kind: "nodeDefinition"; nodeType: string }
  | { kind: "event_graph"; path: string; event: { name: string } }
  | { kind: "function_graph"; path: string; fn: FunctionDetailModel }
  | { kind: "chart"; path: string; name: string; document: ChartDocument | null }
  | { kind: "mind"; path: string; panelInstanceId: string; nodeId: string | null }
  | { kind: "doc"; path: string }
  | { kind: "unavailable"; resourceKind: ResourceKind; resourceRef: string }
  | { kind: "data"; id: string; dataframe: DeepReadonly<DatabaseRecord> };

/** target + 当前目标的只读数据 → Detail 面板判别联合（无回调，纯数据） */
export function resolveDetailPanelModel(input: DetailPanelResolveInput): DetailPanelModel {
  const { target, selectedLog, eventName, functionGraph, dataframe, chartDocument } = input;

  if (!target) return { kind: "empty" };

  switch (target.kind) {
    case "log":
      return selectedLog ? { kind: "log", log: selectedLog } : { kind: "empty" };
    case "node":
      return { kind: "node", nodeId: target.id, graphPath: target.graphPath };
    case "nodeDefinition":
      return { kind: "nodeDefinition", nodeType: target.nodeType };
    case "event_graph": {
      return eventName !== null
        ? { kind: "event_graph", path: target.path, event: { name: eventName } }
        : { kind: "unavailable", resourceKind: "event_graph", resourceRef: target.path };
    }
    case "function_graph": {
      if (!functionGraph)
        return { kind: "unavailable", resourceKind: "function_graph", resourceRef: target.path };
      return {
        kind: "function_graph",
        path: target.path,
        fn: {
          name: functionGraph.name,
          inputs: functionGraph.functionInputs,
          outputs: functionGraph.functionOutputs,
        },
      };
    }
    case "chart":
      return {
        kind: "chart",
        path: target.chartPath,
        name: input.chartName ?? "",
        document: chartDocument,
      };
    case "mind":
      return {
        kind: "mind",
        path: target.path,
        panelInstanceId: target.panelInstanceId,
        nodeId: target.nodeId,
      };
    case "doc":
      return { kind: "doc", path: target.path };
    case "data": {
      return dataframe
        ? { kind: "data", id: target.id, dataframe }
        : { kind: "unavailable", resourceKind: "database", resourceRef: target.id };
    }
    default: {
      const exhaustive: never = target;
      return exhaustive;
    }
  }
}
