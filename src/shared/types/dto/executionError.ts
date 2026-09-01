export type InternalCompilationStageDto = "analysis" | "lowering";

export interface InternalCompilationFailureDto {
  stage: InternalCompilationStageDto;
  code: string;
  nodeId: string | null;
}

export interface InternalCompilationErrorDetailsDto {
  internalCompilationFailure: InternalCompilationFailureDto;
}

const uuidPattern = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function hasExactKeys(value: unknown, keys: readonly string[]): value is Record<string, unknown> {
  if (!isRecord(value)) return false;
  const actual = Object.keys(value);
  return (
    actual.length === keys.length &&
    keys.every((key) => Object.prototype.hasOwnProperty.call(value, key))
  );
}

export function parseInternalCompilationErrorDetails(
  value: unknown,
): InternalCompilationErrorDetailsDto {
  if (!hasExactKeys(value, ["internalCompilationFailure"])) {
    throw new Error("Invalid internal compilation failure response");
  }
  const failure = value.internalCompilationFailure;
  if (
    !hasExactKeys(failure, ["stage", "code", "nodeId"]) ||
    (failure.stage !== "analysis" && failure.stage !== "lowering") ||
    typeof failure.code !== "string" ||
    failure.code.length === 0 ||
    (failure.nodeId !== null &&
      (typeof failure.nodeId !== "string" || !uuidPattern.test(failure.nodeId)))
  ) {
    throw new Error("Invalid internal compilation failure response");
  }
  return {
    internalCompilationFailure: {
      stage: failure.stage,
      code: failure.code,
      nodeId: failure.nodeId,
    },
  };
}
