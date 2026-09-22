import type { DeepReadonly } from "@/shared/types/deepReadonly";
import type { ValueType } from "@/shared/types/domain/valueType";
import type { PinData } from "@/features/domain/editorProjection/graphRuntimeTypes";

export type ConnectionCandidatePin = DeepReadonly<
  Pick<
    PinData,
    "id" | "nodeId" | "direction" | "connections" | "orphan" | "acceptedType" | "typeState"
  >
>;

export type TypeCompatibility = "compatible" | "incompatible" | "indeterminate";

function everyCompatibility(results: TypeCompatibility[]): TypeCompatibility {
  if (results.some((result) => result === "incompatible")) return "incompatible";
  if (results.some((result) => result === "indeterminate")) return "indeterminate";
  return "compatible";
}

function someCompatibility(results: TypeCompatibility[]): TypeCompatibility {
  if (results.some((result) => result === "compatible")) return "compatible";
  if (results.some((result) => result === "indeterminate")) return "indeterminate";
  return "incompatible";
}

export function getDataTypeCompatibility(
  source: DeepReadonly<ValueType> | null | undefined,
  target: DeepReadonly<ValueType> | null | undefined,
): TypeCompatibility {
  if (!source || !target) return "indeterminate";
  if (source.kind === "Scalar" && target.kind === "Scalar")
    return source.inner === target.inner ? "compatible" : "incompatible";
  if (source.kind === "OneOf") {
    return everyCompatibility(
      source.inner.map((member) => getDataTypeCompatibility(member, target)),
    );
  }
  if (target.kind === "OneOf") {
    return someCompatibility(
      target.inner.map((member) => getDataTypeCompatibility(source, member)),
    );
  }
  if (target.kind !== source.kind) return "incompatible";
  if (target.kind === "Array" && source.kind === "Array") {
    return getDataTypeCompatibility(source.inner, target.inner);
  }
  if (target.kind === "DataSeries" && source.kind === "DataSeries") {
    return getDataTypeCompatibility(source.inner, target.inner);
  }
  if (target.kind === "Struct" && source.kind === "Struct") {
    return source.inner === target.inner ? "compatible" : "incompatible";
  }
  return "compatible";
}

function effectiveDomain(pin: ConnectionCandidatePin): DeepReadonly<ValueType[]> | null {
  switch (pin.typeState.status) {
    case "exact":
      return pin.typeState.dataType ? [pin.typeState.dataType] : null;
    case "constrained":
      return pin.typeState.domain;
    case "unknown":
    case "conflict":
      return null;
  }
}

export function getPinCompatibility(
  source: ConnectionCandidatePin,
  target: ConnectionCandidatePin,
): TypeCompatibility {
  if (
    source.id === target.id ||
    source.nodeId === target.nodeId ||
    source.direction !== "output" ||
    target.direction !== "input"
  )
    return "incompatible";

  const sourceDomain = effectiveDomain(source);
  const targetDomain = target.acceptedType.domain;
  if (!sourceDomain || !targetDomain || sourceDomain.length === 0 || targetDomain.length === 0) {
    return "indeterminate";
  }
  const sourceResults = sourceDomain.map((sourceType) =>
    someCompatibility(
      targetDomain.map((targetType) => getDataTypeCompatibility(sourceType, targetType)),
    ),
  );
  if (sourceResults.every((result) => result === "compatible")) return "compatible";
  return sourceResults.some((result) => result !== "incompatible")
    ? "indeterminate"
    : "incompatible";
}

export type ConnectionInvalidReason =
  | "samePort"
  | "sameNode"
  | "directionMismatch"
  | "typeMismatch"
  | "orphan"
  | "capacityReached";

export type ConnectionCompatibility =
  | { kind: "append" }
  | { kind: "replace" }
  | { kind: "invalid"; reason: ConnectionInvalidReason };

function canAppendOrReplace(pin: ConnectionCandidatePin): boolean {
  return pin.connections.canAppend || pin.connections.canReplace;
}

export function resolveConnectionCompatibility(
  a: ConnectionCandidatePin,
  b: ConnectionCandidatePin,
): ConnectionCompatibility {
  if (a.id === b.id) return { kind: "invalid", reason: "samePort" };
  if (a.nodeId === b.nodeId) return { kind: "invalid", reason: "sameNode" };
  if (a.direction === b.direction) return { kind: "invalid", reason: "directionMismatch" };

  const source = a.direction === "output" ? a : b;
  const target = a.direction === "input" ? a : b;
  if (source.orphan || target.orphan) return { kind: "invalid", reason: "orphan" };
  if (!canAppendOrReplace(source) || !canAppendOrReplace(target)) {
    return { kind: "invalid", reason: "capacityReached" };
  }

  if (getPinCompatibility(source, target) === "incompatible") {
    return { kind: "invalid", reason: "typeMismatch" };
  }

  return source.connections.canReplace || target.connections.canReplace
    ? { kind: "replace" }
    : { kind: "append" };
}
