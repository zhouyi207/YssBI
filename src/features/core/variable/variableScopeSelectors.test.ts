import { describe, expect, it } from "vitest";
import {
  partitionVariableCatalog,
  selectLocalVariableEntriesForGraph,
} from "./variableScopeSelectors";
import type { Variable } from "@/shared/types/domain/variable";

function functionVariable(id: string, name: string, functionPath: string): Variable {
  return {
    id,
    name,
    dataType: { kind: "Int64" },
    dataValue: { kind: "Int64", value: 0 },
    description: "",
    scope: { type: "function", functionPath },
    tags: [],
  };
}

describe("variableScopeSelectors", () => {
  it("selects function-scoped variables for a graph path", () => {
    const variables = {
      "local-1": functionVariable("local-1", "Alpha", "functions/A.yssbi-function"),
      "local-2": functionVariable("local-2", "Beta", "functions/B.yssbi-function"),
      "global-1": {
        ...functionVariable("global-1", "Global", "functions/A.yssbi-function"),
        scope: { type: "global" as const },
      },
    };

    expect(
      selectLocalVariableEntriesForGraph(variables, "functions/A.yssbi-function", "function"),
    ).toEqual([{ id: "local-1", name: "Alpha", typeLabel: "Int64", dataType: { kind: "Int64" } }]);
  });

  it("partitions global and scoped local variables for sidebar", () => {
    const variables = {
      "global-1": {
        ...functionVariable("global-1", "Counter", "functions/A.yssbi-function"),
        scope: { type: "global" as const },
      },
      "local-1": functionVariable("local-1", "Temp", "functions/A.yssbi-function"),
    };

    const { global, local } = partitionVariableCatalog(variables, {
      graphPath: "functions/A.yssbi-function",
      graphKind: "function",
    });

    expect(global).toEqual({
      "global-1": {
        id: "global-1",
        name: "Counter",
        typeLabel: "Int64",
        dataType: { kind: "Int64" },
      },
    });
    expect(local).toEqual({
      "local-1": { id: "local-1", name: "Temp", typeLabel: "Int64", dataType: { kind: "Int64" } },
    });
  });
});
