import { produce, type Draft } from "immer";
import { shallow } from "zustand/shallow";
import type { GraphPath, FunctionSignaturePin } from "@/shared/types";
import type { FunctionSignatureDto } from "@/shared/types/domain/editorMutation";
import type { ProjectNodeFileIndexRow } from "@/shared/types/domain/project";
import type { ValueType } from "@/shared/types/domain/valueType";

/** 由 ResourceStore 随资源索引发布的图类型与函数签名投影。 */
export interface GraphMeta {
  type: "event_graph" | "function_graph";
  functionRevision?: number;
  functionSignature?: FunctionSignatureDto;
  functionInputs?: FunctionSignaturePin[];
  functionOutputs?: FunctionSignaturePin[];
}

function updateValueType(current: Draft<ValueType>, incoming: ValueType): ValueType {
  if (
    (current.kind === "Array" || current.kind === "DataSeries") &&
    (incoming.kind === "Array" || incoming.kind === "DataSeries")
  ) {
    current.kind = incoming.kind;
    current.inner = updateValueType(current.inner, incoming.inner);
    return current;
  }
  if (current.kind === "OneOf" && incoming.kind === "OneOf") {
    current.inner.length = incoming.inner.length;
    for (const [index, type] of incoming.inner.entries()) {
      current.inner[index] = current.inner[index]
        ? updateValueType(current.inner[index], type)
        : structuredClone(type);
    }
    return current;
  }
  return shallow(current, incoming) ? current : structuredClone(incoming);
}

function updateFunctionPins(
  current: Draft<FunctionSignaturePin[]>,
  incoming: readonly FunctionSignaturePin[],
): void {
  current.length = incoming.length;
  for (const [index, pin] of incoming.entries()) {
    const previous = current[index];
    if (!previous) {
      current[index] = structuredClone(pin);
      continue;
    }
    previous.id = pin.id;
    previous.name = pin.name;
    previous.dataType = updateValueType(previous.dataType, pin.dataType);
  }
}

export function prepareGraphMetaSnapshot(
  graphs: readonly ProjectNodeFileIndexRow[],
  current: Record<GraphPath, GraphMeta> = {},
): Record<GraphPath, GraphMeta> {
  const paths = new Set(graphs.map((graph) => graph.path));
  return produce(current, (draft) => {
    for (const path of Object.keys(current)) {
      if (!paths.has(path)) delete draft[path];
    }
    for (const graph of graphs) {
      const meta = (draft[graph.path] ??= { type: graph.type });
      meta.type = graph.type;
      if (graph.type === "event_graph") {
        delete meta.functionRevision;
        delete meta.functionSignature;
        delete meta.functionInputs;
        delete meta.functionOutputs;
        continue;
      }

      meta.functionRevision = graph.functionEditorProjection.functionRevision;
      if (!meta.functionSignature) {
        meta.functionSignature = structuredClone(graph.functionSignature);
      } else {
        const signature = meta.functionSignature;
        signature.return_type = graph.functionSignature.return_type;
        signature.parameters.length = graph.functionSignature.parameters.length;
        for (const [index, parameter] of graph.functionSignature.parameters.entries()) {
          if (!shallow(signature.parameters[index], parameter))
            signature.parameters[index] = { ...parameter };
        }
      }
      if (!meta.functionInputs)
        meta.functionInputs = structuredClone(graph.functionEditorProjection.inputs);
      else updateFunctionPins(meta.functionInputs, graph.functionEditorProjection.inputs);
      if (!meta.functionOutputs)
        meta.functionOutputs = structuredClone(graph.functionEditorProjection.outputs);
      else updateFunctionPins(meta.functionOutputs, graph.functionEditorProjection.outputs);
    }
  });
}
