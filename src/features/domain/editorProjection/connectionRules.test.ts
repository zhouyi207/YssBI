import { describe, it, expect } from "vitest";
import type { PinDirection } from "@/shared/types/domain/pin";
import { dataTypeDisplay, type ValueType } from "@/shared/types/domain/valueType";
import {
  resolveConnectionCompatibility,
  getDataTypeCompatibility,
  getPinCompatibility,
  type ConnectionCandidatePin,
} from "./connectionRules";

const NUMERIC: ValueType = { kind: "Scalar", inner: "Numeric" };
const TEXT: ValueType = { kind: "Scalar", inner: "Text" };
const NUMERIC_SERIES: ValueType = {
  kind: "DataSeries",
  inner: { kind: "Scalar", inner: "Numeric" },
};
const LINEAR_MODEL: ValueType = { kind: "Struct", inner: "statistics.model.linear" };
const LINEAR_RESULT: ValueType = { kind: "Struct", inner: "statistics.result.linear" };

function pin(
  partial: Partial<ConnectionCandidatePin> & { direction: PinDirection; dataType?: ValueType },
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

describe("semantic type display", () => {
  it("displays all seven meanings independently of physical representation", () => {
    for (const inner of [
      "Numeric",
      "Categorical",
      "Ordinal",
      "Binary",
      "Datetime",
      "Text",
      "Identifier",
    ] as const) {
      expect(dataTypeDisplay({ kind: "Scalar", inner })).toBe(inner);
      expect(
        getDataTypeCompatibility({ kind: "Scalar", inner }, { kind: "Scalar", inner: "Numeric" }),
      ).toBe(inner === "Numeric" ? "compatible" : "incompatible");
    }
  });
  it("keeps series structure and semantic unions explicit", () => {
    expect(
      dataTypeDisplay({ kind: "DataSeries", inner: { kind: "Scalar", inner: "Numeric" } }),
    ).toBe("DataSeries<Numeric>");
    expect(
      dataTypeDisplay({
        kind: "OneOf",
        inner: [
          { kind: "Scalar", inner: "Numeric" },
          { kind: "Scalar", inner: "Text" },
        ],
      }),
    ).toBe("Numeric | Text");
  });
});

describe("getDataTypeCompatibility", () => {
  it("requires every source union member to be assignable", () => {
    expect(
      getDataTypeCompatibility(
        {
          kind: "OneOf",
          inner: [
            { kind: "Scalar", inner: "Numeric" },
            { kind: "Scalar", inner: "Text" },
          ],
        },
        { kind: "Scalar", inner: "Numeric" },
      ),
    ).toBe("incompatible");
  });

  it("accepts when every source union member is assignable", () => {
    expect(
      getDataTypeCompatibility(
        {
          kind: "OneOf",
          inner: [
            { kind: "Scalar", inner: "Numeric" },
            { kind: "Scalar", inner: "Text" },
          ],
        },
        {
          kind: "OneOf",
          inner: [
            { kind: "Scalar", inner: "Numeric" },
            { kind: "Scalar", inner: "Text" },
          ],
        },
      ),
    ).toBe("compatible");
  });

  it("returns indeterminate when either projected type is missing", () => {
    expect(getDataTypeCompatibility(null, { kind: "Scalar", inner: "Numeric" })).toBe(
      "indeterminate",
    );
    expect(getDataTypeCompatibility({ kind: "Scalar", inner: "Numeric" }, undefined)).toBe(
      "indeterminate",
    );
  });

  it("accepts numeric series into a union containing numeric series", () => {
    const target = {
      kind: "OneOf",
      inner: [
        { kind: "DataSeries", inner: { kind: "Scalar", inner: "Text" } },
        { kind: "DataSeries", inner: { kind: "Scalar", inner: "Numeric" } },
      ],
    } satisfies ValueType;

    expect(getDataTypeCompatibility(NUMERIC_SERIES, target)).toBe("compatible");
  });

  it("does not treat Any as a wildcard", () => {
    expect(getDataTypeCompatibility({ kind: "Any" }, NUMERIC)).toBe("incompatible");
    expect(getDataTypeCompatibility(NUMERIC, { kind: "Any" })).toBe("incompatible");
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
      acceptedType: { display: "core.numeric", domain: [NUMERIC] },
      typeState: { status: "exact", display: "core.numeric", dataType: NUMERIC },
      dataType: NUMERIC,
    });

    expect(getPinCompatibility(output, input)).toBe("indeterminate");
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
    dataType: NUMERIC,
    connections: appendCapability,
  });
  const input = pin({
    id: "input",
    nodeId: "target",
    direction: "input",
    dataType: NUMERIC,
    connections: appendCapability,
  });

  it("returns append for compatible append-capable endpoints", () => {
    expect(resolveConnectionCompatibility(output, input)).toEqual({ kind: "append" });
  });

  it("matches output -> input of the same structured type", () => {
    const out = pin({ id: "o", nodeId: "a", direction: "output", dataType: NUMERIC_SERIES });
    const inSeries = pin({ id: "i", nodeId: "b", direction: "input", dataType: NUMERIC_SERIES });
    const inScalar = pin({ id: "i2", nodeId: "b", direction: "input", dataType: NUMERIC });
    expect(resolveConnectionCompatibility(inSeries, out)).toEqual({ kind: "append" });
    expect(resolveConnectionCompatibility(inScalar, out)).toEqual({
      kind: "invalid",
      reason: "typeMismatch",
    });
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
        dataType: TEXT,
        acceptedType: { display: "Text", domain: [TEXT] },
        typeState: { status: "exact", display: "Text", dataType: TEXT },
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

  it("matches nominal types by identity independently of endpoint argument order", () => {
    const modelOutput = pin({
      id: "modelOut",
      nodeId: "ols",
      direction: "output",
      dataType: LINEAR_MODEL,
    });
    const modelInput = pin({
      id: "modelIn",
      nodeId: "predict",
      direction: "input",
      dataType: LINEAR_MODEL,
    });
    const resultInput = pin({
      id: "resultIn",
      nodeId: "consumer",
      direction: "input",
      dataType: LINEAR_RESULT,
    });

    expect(resolveConnectionCompatibility(modelOutput, modelInput)).toEqual({
      kind: "append",
    });
    expect(resolveConnectionCompatibility(modelInput, modelOutput)).toEqual({
      kind: "append",
    });
    expect(resolveConnectionCompatibility(modelOutput, resultInput)).toEqual({
      kind: "invalid",
      reason: "typeMismatch",
    });
  });
});
