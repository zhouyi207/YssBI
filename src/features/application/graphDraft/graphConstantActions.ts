import { useGraphDraftStore, getGraphDraftDocument } from "@/features/core/graphDraft";
import type { GraphConstantDto } from "@/shared/types/domain/editorMutation";
import type { DataType } from "@/shared/types/domain/dataType";
import { getDefaultValue } from "@/shared/types/domain/dataType";
import type { DataValue } from "@/shared/types/domain/dataValue";
import {
  deserializeDataValue,
  dataValueFromRaw,
  serializeDataValue,
} from "@/shared/types/domain/dataValue";
import { applyGraphDraftMutation } from "./graphDraftCoordinator";

const EMPTY_CONSTANTS: Record<string, GraphConstantDto> = {};

export function useGraphConstants(graphPath: string) {
  const constants = useGraphDraftStore(
    (state) => state.sessions[graphPath]?.document.constants ?? EMPTY_CONSTANTS,
  );
  const loaded = useGraphDraftStore((state) => Boolean(state.sessions[graphPath]));
  const saving = useGraphDraftStore((state) => state.sessions[graphPath]?.saving === true);
  return { constants, loaded, saving };
}

export function editableConstantValue(constant: GraphConstantDto): DataValue {
  if (constant.tabular) {
    const literal = JSON.stringify(constant.tabular.columns);
    if (constant.dataType.kind === "DataFrame") return { kind: "DataFrame", value: literal };
    if (constant.dataType.kind === "DataSeries") return { kind: "DataSeries", value: literal };
  }
  return deserializeDataValue(constant.dataValue);
}

export function createGraphConstant(graphPath: string, baseName: string) {
  return applyGraphDraftMutation({
    graphPath,
    mutation: (document) => {
      const names = new Set(Object.values(document.constants ?? {}).map((value) => value.name));
      let name = baseName;
      for (let index = 2; names.has(name); index++) name = `${baseName} ${index}`;
      const id = crypto.randomUUID();
      return {
        type: "setConstant",
        payload: {
          id,
          constant: { id, name, dataType: { kind: "Int64" }, dataValue: { Int64: 0 } },
        },
      };
    },
  });
}

export function updateGraphConstant(
  graphPath: string,
  id: string,
  patch: { name?: string; dataType?: DataType; dataValue?: DataValue },
) {
  return applyGraphDraftMutation({
    graphPath,
    mutation: (document) => {
      const current = document.constants?.[id];
      if (!current) throw new Error("Graph constant is unavailable");
      const constant = {
        ...current,
        ...(patch.name === undefined ? {} : { name: patch.name }),
        ...(patch.dataType ? { dataType: patch.dataType } : {}),
      };
      if (patch.dataValue !== undefined) {
        const value = serializeDataValue(patch.dataValue);
        const previous = current.dataValue;
        if (
          typeof value === "object" &&
          "DataSeries" in value &&
          typeof previous === "object" &&
          "DataSeries" in previous &&
          typeof previous.DataSeries === "object"
        ) {
          value.DataSeries = {
            ...previous.DataSeries,
            ...(typeof value.DataSeries === "string" ? { id: value.DataSeries } : value.DataSeries),
          };
        }
        constant.dataValue = value;
      } else if (patch.dataType) {
        constant.dataValue = serializeDataValue(
          dataValueFromRaw(getDefaultValue(patch.dataType), patch.dataType),
        );
        delete constant.tabular;
      }
      return { type: "setConstant", payload: { id, constant } };
    },
  });
}

export function deleteGraphConstant(graphPath: string, id: string) {
  return applyGraphDraftMutation({
    graphPath,
    mutation: { type: "setConstant", payload: { id, constant: null } },
  });
}

export function insertConstantReference(graphPath: string, id: string) {
  const nodes = Object.values(getGraphDraftDocument(graphPath)?.nodes ?? {});
  const position = nodes.length
    ? {
        x: Math.min(...nodes.map((node) => node.position.x)) - 240,
        y: Math.min(...nodes.map((node) => node.position.y)),
      }
    : { x: 80, y: 80 };
  return applyGraphDraftMutation({
    graphPath,
    mutation: { type: "insertConstantReference", payload: { id, position } },
  });
}
