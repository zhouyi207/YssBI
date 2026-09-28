import type { DetailTarget } from "@/features/core/editor/detail/detailTypes";
import type { ResourceKind } from "@/shared/types/domain/resource";
import type { FunctionResourceView } from "@/features/core/resource/functionResourceView";
import type { GraphResourceRecord } from "@/features/core/resource/resourceSelectors";
import type { FunctionPinSpec } from "@/shared/types/domain/graph";
import type { ChartDocument } from "@/shared/types/domain/chart";
import type { DatabaseRecord } from "@/shared/types/domain/database";
import type { LogRecordDto } from "@/shared/types/domain/log";

export interface DetailCatalogSnapshot {
  eventGraphs: GraphResourceRecord;
  /** 已合并名称 + 签名（`useFunctionCatalog` / `FunctionResourceView`） */
  functionGraphs: Record<string, FunctionResourceView>;
  dataframes: Record<string, DatabaseRecord>;
}

export interface DetailPanelResolveInput extends DetailCatalogSnapshot {
  target: DetailTarget | null;
  selectedLog: LogRecordDto | null;
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
  | { kind: "data"; id: string; dataframe: DatabaseRecord };

/** target + 目录快照 → Detail 面板判别联合（无回调，纯数据） */
export function resolveDetailPanelModel(input: DetailPanelResolveInput): DetailPanelModel {
  const { target, selectedLog, eventGraphs, functionGraphs, dataframes, chartDocument } = input;

  if (!target) return { kind: "empty" };

  switch (target.kind) {
    case "log":
      return selectedLog ? { kind: "log", log: selectedLog } : { kind: "empty" };
    case "node":
      return { kind: "node", nodeId: target.id, graphPath: target.graphPath };
    case "nodeDefinition":
      return { kind: "nodeDefinition", nodeType: target.nodeType };
    case "event_graph": {
      const event = eventGraphs[target.path];
      return event
        ? { kind: "event_graph", path: target.path, event: { name: event.name } }
        : { kind: "unavailable", resourceKind: "event_graph", resourceRef: target.path };
    }
    case "function_graph": {
      const fnRecord = functionGraphs[target.path];
      if (!fnRecord)
        return { kind: "unavailable", resourceKind: "function_graph", resourceRef: target.path };
      return {
        kind: "function_graph",
        path: target.path,
        fn: {
          name: fnRecord.name,
          inputs: fnRecord.functionInputs,
          outputs: fnRecord.functionOutputs,
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
      const dataframe = dataframes[target.id];
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
