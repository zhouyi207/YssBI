import { describe, it, expect } from "vitest";
import type { PinDirection } from "@/shared/types/domain/pin";
import { dataTypeDisplay, type DataType } from "@/shared/types/domain/dataType";
import type { TypeSystemSnapshot } from "@/shared/types/domain/typeSystem";
import {
  isPinCompatible,
  resolveConnectionCompatibility,
  getDataTypeCompatibility,
  getPinCompatibility,
  type ConnectionCandidatePin,
} from "./connectionRules";

const FLOAT64: DataType = { kind: "Float64" };
const STRING: DataType = { kind: "String" };
const SERIES_FLOAT64: DataType = { kind: "DataSeries", inner: { kind: "Float64" } };
const MODEL: DataType = { kind: "Struct", inner: "Model" };
const OLS_MODEL: DataType = { kind: "Struct", inner: "OLSModel" };
const OLS_RESULT: DataType = { kind: "Struct", inner: "OLSResult" };

const TYPE_SYSTEM: TypeSystemSnapshot = {
  structTypes: {
    Model: { key: "Model", parents: [], category: "model" },
    OLSModel: { key: "OLSModel", parents: ["Model"], category: "model" },
    OLSResult: { key: "OLSResult", parents: [], category: "result" },
  },
};

function pin(
  partial: Partial<ConnectionCandidatePin> & { direction: PinDirection; dataType?: DataType },
): ConnectionCandidatePin {
  const { dataType, ...projected } = partial;
  const acceptedType =
    partial.acceptedType ??
    (dataType ? { display: dataType.kind, domain: [dataType] } : { display: "T", domain: null });
  const typeState =
    partial.typeState ??
    (dataType
      ? { status: "exact" as const, display: dataType.kind, dataType }
      : { status: "unknown" as const, reasonCode: "unresolved_upstream" });
  return {
    ...projected,
    id: partial.id ?? "p1",
    nodeId: partial.nodeId ?? "n1",
    direction: partial.direction,
    orphan: partial.orphan ?? false,
    connections: partial.connections ?? {
      current: 0,
      maximum: null,
      ordered: false,
      canAppend: true,
      canReplace: false,
      canMove: true,
    },
    acceptedType,
    typeState,
  };
}

describe("dataTypeDisplay Number alias", () => {
  it("uses Number only for the exact scalar numeric union", () => {
    expect(
      dataTypeDisplay({
        kind: "OneOf",
        inner: [{ kind: "Float64" }, { kind: "Int64" }],
      }),
    ).toBe("Number");
    expect(
      dataTypeDisplay({
        kind: "OneOf",
        inner: [{ kind: "Float64" }, { kind: "String" }],
      }),
    ).toBe("Float64 | String");
  });

  it("uses DataSeries<Number> only for the exact outer numeric series union", () => {
    expect(
      dataTypeDisplay({
        kind: "OneOf",
        inner: [
          { kind: "DataSeries", inner: { kind: "Int64" } },
          { kind: "DataSeries", inner: { kind: "Float64" } },
        ],
      }),
    ).toBe("DataSeries<Number>");
    expect(
      dataTypeDisplay({
        kind: "DataSeries",
        inner: { kind: "OneOf", inner: [{ kind: "Int64" }, { kind: "Float64" }] },
      }),
    ).not.toBe("DataSeries<Number>");
  });
});

describe("getDataTypeCompatibility", () => {
  it("requires every source union member to be assignable", () => {
    expect(
      getDataTypeCompatibility(
        { kind: "OneOf", inner: [{ kind: "Int64" }, { kind: "String" }] },
        { kind: "Int64" },
      ),
    ).toBe("incompatible");
  });

  it("accepts when every source union member is assignable", () => {
    expect(
      getDataTypeCompatibility(
        { kind: "OneOf", inner: [{ kind: "Int64" }, { kind: "Float64" }] },
        { kind: "OneOf", inner: [{ kind: "Float64" }, { kind: "Int64" }] },
      ),
    ).toBe("compatible");
  });

  it("returns indeterminate when either projected type is missing", () => {
    expect(getDataTypeCompatibility(null, { kind: "Float64" })).toBe("indeterminate");
    expect(getDataTypeCompatibility({ kind: "Float64" }, undefined)).toBe("indeterminate");
  });

  it("accepts homogeneous numeric series into DataSeries Number union", () => {
    const target = {
      kind: "OneOf",
      inner: [
        { kind: "DataSeries", inner: { kind: "Int64" } },
        { kind: "DataSeries", inner: { kind: "Float64" } },
      ],
    } satisfies DataType;

    expect(getDataTypeCompatibility(SERIES_FLOAT64, target)).toBe("compatible");
  });

  it("does not treat Any as a wildcard", () => {
    expect(getDataTypeCompatibility({ kind: "Any" }, FLOAT64)).toBe("incompatible");
    expect(getDataTypeCompatibility(FLOAT64, { kind: "Any" })).toBe("incompatible");
  });
});

describe("getPinCompatibility", () => {
  it("returns indeterminate for unresolved projected pins", () => {
    const output = pin({
      id: "output",
      nodeId: "source",
      direction: "output",
      typeState: { status: "unknown", reasonCode: "unresolved_upstream" },
    });
    const input = pin({
      id: "input",
      nodeId: "target",
      direction: "input",
      acceptedType: { display: "core.float64", domain: [FLOAT64] },
      typeState: { status: "exact", display: "core.float64", dataType: FLOAT64 },
      dataType: FLOAT64,
    });

    expect(getPinCompatibility(output, input)).toBe("indeterminate");
  });

  it("uses the resolved source domain without treating a constrained Pin as an exact Union", () => {
    const output = pin({
      id: "output",
      nodeId: "source",
      direction: "output",
      typeState: {
        status: "constrained",
        display: "core.int64 | core.float64",
        domain: [{ kind: "Int64" }, FLOAT64],
      },
    });
    const floatInput = pin({
      id: "float-input",
      nodeId: "float-target",
      direction: "input",
      acceptedType: { display: "core.float64", domain: [FLOAT64] },
    });
    const intInput = pin({
      id: "int-input",
      nodeId: "int-target",
      direction: "input",
      acceptedType: { display: "core.int64", domain: [{ kind: "Int64" }] },
    });

    expect(getPinCompatibility(output, floatInput)).toBe("compatible");
    expect(getPinCompatibility(output, intInput)).toBe("indeterminate");
  });
});

describe("isPinCompatible", () => {
  it("matches output -> input of the same structured type", () => {
    const out = pin({ id: "o", nodeId: "a", direction: "output", dataType: SERIES_FLOAT64 });
    const inSeries = pin({ id: "i", nodeId: "b", direction: "input", dataType: SERIES_FLOAT64 });
    const inScalar = pin({ id: "i2", nodeId: "b", direction: "input", dataType: FLOAT64 });
    expect(isPinCompatible(inSeries, out)).toBe(true);
    expect(isPinCompatible(inScalar, out)).toBe(false);
  });
});

describe("resolveConnectionCompatibility", () => {
  const appendCapability = {
    current: 0,
    maximum: 1,
    ordered: false,
    canAppend: true,
    canReplace: false,
    canMove: false,
  };

  const output = pin({
    id: "output",
    nodeId: "source",
    direction: "output",
    dataType: FLOAT64,
    connections: appendCapability,
  });
  const input = pin({
    id: "input",
    nodeId: "target",
    direction: "input",
    dataType: FLOAT64,
    connections: appendCapability,
  });

  it("returns append for compatible append-capable endpoints", () => {
    expect(resolveConnectionCompatibility(output, input)).toEqual({ kind: "append" });
  });

  it("returns replace without displaced connection IDs", () => {
    const replaceable = pin({
      ...input,
      connections: {
        ...appendCapability,
        current: 1,
        canAppend: false,
        canReplace: true,
      },
    });

    expect(resolveConnectionCompatibility(output, replaceable)).toEqual({ kind: "replace" });
  });

  it.each([
    ["samePort", output, output],
    ["sameNode", output, pin({ ...input, nodeId: output.nodeId })],
    ["directionMismatch", output, pin({ ...input, direction: "output" })],
    [
      "typeMismatch",
      output,
      pin({
        ...input,
        dataType: STRING,
        acceptedType: { display: "String", domain: [STRING] },
        typeState: { status: "exact", display: "String", dataType: STRING },
      }),
    ],
    ["orphan", output, pin({ ...input, orphan: true })],
    [
      "capacityReached",
      output,
      pin({
        ...input,
        connections: {
          ...appendCapability,
          current: 1,
          canAppend: false,
          canReplace: false,
        },
      }),
    ],
  ] as const)("returns the %s invalid reason", (reason, source, target) => {
    expect(resolveConnectionCompatibility(source, target)).toEqual({ kind: "invalid", reason });
  });

  it("preserves structured data type compatibility and argument-order independence", () => {
    const modelOutput = pin({
      id: "modelOut",
      nodeId: "ols",
      direction: "output",
      dataType: OLS_MODEL,
    });
    const modelInput = pin({
      id: "modelIn",
      nodeId: "predict",
      direction: "input",
      dataType: MODEL,
    });
    const resultInput = pin({
      id: "resultIn",
      nodeId: "consumer",
      direction: "input",
      dataType: OLS_RESULT,
    });

    expect(resolveConnectionCompatibility(modelOutput, modelInput, TYPE_SYSTEM)).toEqual({
      kind: "append",
    });
    expect(resolveConnectionCompatibility(modelInput, modelOutput, TYPE_SYSTEM)).toEqual({
      kind: "append",
    });
    expect(resolveConnectionCompatibility(modelOutput, resultInput, TYPE_SYSTEM)).toEqual({
      kind: "invalid",
      reason: "typeMismatch",
    });
  });
});
