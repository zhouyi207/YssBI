import type { DetailTarget } from "@/features/core/editor/detail/detailTypes";
import type { FunctionResourceView } from "@/features/core/resource/functionResourceView";
import type { GraphResourceRecord } from "@/features/core/resource/resourceSelectors";
import type { FunctionPinSpec } from "@/shared/types/domain/graph";
import type { Variable } from "@/shared/types/domain/variable";
import type { ChartDocument } from "@/shared/types/domain/chart";
import type { DatabaseRecord } from "@/shared/types/domain/database";
import type { DiagnosticRecordDto } from "@/shared/types/domain/diagnostics";

export interface DetailCatalogSnapshot {
  variables: Record<string, Variable>;
  events: GraphResourceRecord;
  /** 已合并名称 + 签名（`useFunctionCatalog` / `FunctionResourceView`） */
  functions: Record<string, FunctionResourceView>;
  dataframes: Record<string, DatabaseRecord>;
}

export interface DetailPanelResolveInput extends DetailCatalogSnapshot {
  target: DetailTarget | null;
  selectedLog: DiagnosticRecordDto | null;
  chartDocument: ChartDocument | null;
}

export type FunctionDetailModel = {
  name: string;
  inputs: FunctionPinSpec[];
  outputs: FunctionPinSpec[];
};

export type DetailPanelModel =
  | { kind: "empty" }
  | { kind: "log"; log: DiagnosticRecordDto }
  | { kind: "node"; nodeId: string; graphPath: string }
  | { kind: "nodeDefinition"; nodeType: string }
  | { kind: "variable"; id: string; variable: Variable }
  | { kind: "event"; path: string; event: { name: string } }
  | { kind: "function"; path: string; fn: FunctionDetailModel }
  | { kind: "chart"; document: ChartDocument }
  | { kind: "data"; id: string; dataframe: DatabaseRecord };

/** target + 目录快照 → Detail 面板判别联合（无回调，纯数据） */
export function resolveDetailPanelModel(input: DetailPanelResolveInput): DetailPanelModel {
  const { target, selectedLog, variables, events, functions, dataframes, chartDocument } = input;

  if (!target) return { kind: "empty" };

  switch (target.kind) {
    case "log":
      return selectedLog ? { kind: "log", log: selectedLog } : { kind: "empty" };
    case "node":
      return { kind: "node", nodeId: target.id, graphPath: target.graphPath };
    case "nodeDefinition":
      return { kind: "nodeDefinition", nodeType: target.nodeType };
    case "variable": {
      const variable = variables[target.id];
      return variable ? { kind: "variable", id: target.id, variable } : { kind: "empty" };
    }
    case "event": {
      const event = events[target.path];
      return event
        ? { kind: "event", path: target.path, event: { name: event.name } }
        : { kind: "empty" };
    }
    case "function": {
      const fnRecord = functions[target.path];
      if (!fnRecord) return { kind: "empty" };
      return {
        kind: "function",
        path: target.path,
        fn: {
          name: fnRecord.name,
          inputs: fnRecord.functionInputs,
          outputs: fnRecord.functionOutputs,
        },
      };
    }
    case "chart":
      return chartDocument ? { kind: "chart", document: chartDocument } : { kind: "empty" };
    case "data": {
      const dataframe = dataframes[target.id];
      return dataframe ? { kind: "data", id: target.id, dataframe } : { kind: "empty" };
    }
    default: {
      const exhaustive: never = target;
      return exhaustive;
    }
  }
}
