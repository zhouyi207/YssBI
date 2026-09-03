import type { DataType } from "@/shared/types/domain/dataType";
import { EMPTY_TYPE_SYSTEM, type TypeSystemSnapshot } from "@/shared/types/domain/typeSystem";
import { structCanAccept } from "@/shared/types/domain/typeSystem";
import type { PinData } from "@/features/domain/editorProjection/graphRuntimeTypes";

export type ConnectionCandidatePin = Pick<
  PinData,
  | "id"
  | "nodeId"
  | "type"
  | "direction"
  | "dataType"
  | "connections"
  | "kind"
  | "orphan"
  | "resolvedType"
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
  source: DataType | null | undefined,
  target: DataType | null | undefined,
  typeSystem: TypeSystemSnapshot = EMPTY_TYPE_SYSTEM,
): TypeCompatibility {
  if (!source || !target) return "indeterminate";
  if (source.kind === "OneOf") {
    return everyCompatibility(
      source.inner.map((member) => getDataTypeCompatibility(member, target, typeSystem)),
    );
  }
  if (target.kind === "OneOf") {
    return someCompatibility(
      target.inner.map((member) => getDataTypeCompatibility(source, member, typeSystem)),
    );
  }
  if (target.kind !== source.kind) return "incompatible";
  if (target.kind === "Array" && source.kind === "Array") {
    return getDataTypeCompatibility(source.inner, target.inner, typeSystem);
  }
  if (target.kind === "DataSeries" && source.kind === "DataSeries") {
    return getDataTypeCompatibility(source.inner, target.inner, typeSystem);
  }
  if (target.kind === "Struct" && source.kind === "Struct") {
    return structCanAccept(target.inner, source.inner, typeSystem) ? "compatible" : "incompatible";
  }
  return "compatible";
}

export function isPinCompatible(
  candidate: ConnectionCandidatePin,
  dragged: ConnectionCandidatePin,
  typeSystem: TypeSystemSnapshot = EMPTY_TYPE_SYSTEM,
): boolean {
  const source = candidate.direction === "output" ? candidate : dragged;
  const target = candidate.direction === "input" ? candidate : dragged;
  return getPinCompatibility(source, target, typeSystem) === "compatible";
}

function projectedPinDataType(pin: ConnectionCandidatePin): DataType | null | undefined {
  if (pin.resolvedType) {
    return pin.resolvedType.resolved ? pin.resolvedType.dataType : null;
  }
  return pin.dataType;
}

export function getPinCompatibility(
  source: ConnectionCandidatePin,
  target: ConnectionCandidatePin,
  typeSystem: TypeSystemSnapshot = EMPTY_TYPE_SYSTEM,
): TypeCompatibility {
  if (
    source.id === target.id ||
    source.nodeId === target.nodeId ||
    source.direction !== "output" ||
    target.direction !== "input"
  )
    return "incompatible";

  if (source.kind !== target.kind) return "incompatible";
  if (source.kind !== "data") return "compatible";
  return getDataTypeCompatibility(
    projectedPinDataType(source),
    projectedPinDataType(target),
    typeSystem,
  );
}

export type ConnectionInvalidReason =
  | "samePort"
  | "sameNode"
  | "directionMismatch"
  | "kindMismatch"
  | "typeMismatch"
  | "orphan"
  | "capacityReached";

export type ConnectionCompatibility =
  | { kind: "append" }
  | { kind: "replace" }
  | { kind: "invalid"; reason: ConnectionInvalidReason };

function connectionKind(pin: ConnectionCandidatePin): "data" | "control" | "effect" {
  return pin.kind;
}

function canAppendOrReplace(pin: ConnectionCandidatePin): boolean {
  return pin.connections.canAppend || pin.connections.canReplace;
}

export function resolveConnectionCompatibility(
  a: ConnectionCandidatePin,
  b: ConnectionCandidatePin,
  typeSystem: TypeSystemSnapshot = EMPTY_TYPE_SYSTEM,
): ConnectionCompatibility {
  if (a.id === b.id) return { kind: "invalid", reason: "samePort" };
  if (a.nodeId === b.nodeId) return { kind: "invalid", reason: "sameNode" };
  if (a.direction === b.direction) return { kind: "invalid", reason: "directionMismatch" };

  const source = a.direction === "output" ? a : b;
  const target = a.direction === "input" ? a : b;
  const sourceKind = connectionKind(source);
  const targetKind = connectionKind(target);

  if (sourceKind !== targetKind) return { kind: "invalid", reason: "kindMismatch" };
  if (source.orphan || target.orphan) return { kind: "invalid", reason: "orphan" };
  if (!canAppendOrReplace(source) || !canAppendOrReplace(target)) {
    return { kind: "invalid", reason: "capacityReached" };
  }

  if (sourceKind === "data" && getPinCompatibility(source, target, typeSystem) === "incompatible") {
    return { kind: "invalid", reason: "typeMismatch" };
  }

  return source.connections.canReplace || target.connections.canReplace
    ? { kind: "replace" }
    : { kind: "append" };
}
