import { useGraphEditingStore, getGraphDocumentProjection } from "@/features/core/graphEditing";
import type { GraphConstantDto } from "@/shared/types/domain/editorMutation";
import type { ValueType } from "@/shared/types/domain/valueType";
import { getDefaultValue } from "@/shared/types/domain/valueType";
import type { DataValue } from "@/shared/types/domain/dataValue";
import {
  deserializeDataValue,
  dataValueFromRaw,
  serializeDataValue,
} from "@/shared/types/domain/dataValue";
import { applyGraphMutation } from "./graphEditCoordinator";

const EMPTY_CONSTANTS: Record<string, GraphConstantDto> = {};

export function useGraphConstants(graphPath: string) {
  const constants = useGraphEditingStore(
    (state) => state.sessions[graphPath]?.document.constants ?? EMPTY_CONSTANTS,
  );
  const loaded = useGraphEditingStore((state) => Boolean(state.sessions[graphPath]));
  const saving = useGraphEditingStore((state) => state.sessions[graphPath]?.saving === true);
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
  return applyGraphMutation({
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
          constant: {
            id,
            name,
            dataType: { kind: "Scalar", inner: "Numeric" },
            dataValue: { Integer: "0" },
          },
        },
      };
    },
  });
}

export function updateGraphConstant(
  graphPath: string,
  id: string,
  patch: { name?: string; dataType?: ValueType; dataValue?: DataValue },
) {
  return applyGraphMutation({
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
        constant.dataValue = serializeDataValue(patch.dataValue);
        delete constant.tabular;
      } else if (
        patch.dataType &&
        JSON.stringify(patch.dataType) !== JSON.stringify(current.dataType)
      ) {
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
  return applyGraphMutation({
    graphPath,
    mutation: { type: "setConstant", payload: { id, constant: null } },
  });
}

export function insertConstantReference(graphPath: string, id: string) {
  const nodes = Object.values(getGraphDocumentProjection(graphPath)?.nodes ?? {});
  const position = nodes.length
    ? {
        x: Math.min(...nodes.map((node) => node.position.x)) - 240,
        y: Math.min(...nodes.map((node) => node.position.y)),
      }
    : { x: 80, y: 80 };
  return applyGraphMutation({
    graphPath,
    mutation: { type: "insertConstantReference", payload: { id, position } },
  });
}
