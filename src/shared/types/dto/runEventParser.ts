import {
  EXECUTION_DEMAND_TYPES,
  type ExecutionDemandDto,
  type GraphOutputRefDto,
} from "./executionDemand";
import { isGraphResourcePath, isPortAddressDto, isUuid } from "./editorProjectionGuards";
import {
  RUN_ERROR_CODES,
  RUN_EVENT_KIND_TYPES,
  RUN_PHASES,
  type GraphRunIdentityDto,
  type RunErrorCode,
  type RunErrorOutcome,
  type RunEvent,
  type RunEventKind,
  type RunPhase,
  type ResultInspectionSource,
} from "./runEvent";

type UnknownRecord = Record<string, unknown>;

const POSITIVE_DECIMAL_ID_PATTERN = /^[1-9]\d*$/;

function isRecord(value: unknown): value is UnknownRecord {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function hasExactKeys(value: UnknownRecord, keys: readonly string[]): boolean {
  return (
    Object.keys(value).length === keys.length &&
    keys.every((key) => Object.prototype.hasOwnProperty.call(value, key))
  );
}

function fail(contract: string): never {
  throw new Error(`Invalid ${contract}`);
}

function assertNever(value: never): never {
  return fail(`unhandled discriminant ${String(value)}`);
}

function parseDiscriminant<T extends string>(
  value: unknown,
  inventory: Readonly<Record<T, true>>,
  contract: string,
): T {
  if (typeof value !== "string" || !Object.prototype.hasOwnProperty.call(inventory, value))
    return fail(contract);
  return value as T;
}

function isPositiveDecimalId(value: unknown): value is string {
  return typeof value === "string" && POSITIVE_DECIMAL_ID_PATTERN.test(value);
}

function parseGraphOutputRefDto(value: unknown): GraphOutputRefDto {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, ["graphPath", "port"]) ||
    !isGraphResourcePath(value.graphPath) ||
    !isPortAddressDto(value.port)
  ) {
    return fail("graph output reference");
  }
  return { graphPath: value.graphPath, port: value.port };
}

export function parseExecutionDemandDto(value: unknown): ExecutionDemandDto {
  if (!isRecord(value)) return fail("execution demand");
  const type = parseDiscriminant(value.type, EXECUTION_DEMAND_TYPES, "execution demand variant");
  switch (type) {
    case "default":
      if (!hasExactKeys(value, ["type"])) return fail("default execution demand");
      return { type: "default" };
    case "outputs":
      if (
        !hasExactKeys(value, ["type", "outputs", "includeDefaultResults", "reuseInputs"]) ||
        !Array.isArray(value.outputs) ||
        typeof value.includeDefaultResults !== "boolean" ||
        typeof value.reuseInputs !== "boolean"
      ) {
        return fail("outputs execution demand");
      }
      return {
        type: "outputs",
        outputs: value.outputs.map(parseGraphOutputRefDto),
        includeDefaultResults: value.includeDefaultResults,
        reuseInputs: value.reuseInputs,
      };
    default:
      return assertNever(type);
  }
}

function parseRunErrorCode(value: unknown): RunErrorCode {
  return parseDiscriminant(value, RUN_ERROR_CODES, "run error code");
}

function parseRunPhase(value: unknown): RunPhase {
  return parseDiscriminant(value, RUN_PHASES, "run phase");
}

function parseErrorOutcome(value: UnknownRecord): RunErrorOutcome {
  return {
    code: parseRunErrorCode(value.code),
    phase: parseRunPhase(value.phase),
    source: value.source === null ? null : parseResultInspectionSource(value.source),
  };
}

function parseGraphRunIdentityDto(value: unknown): GraphRunIdentityDto {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, ["executionSessionId", "graphPath", "runId"]) ||
    typeof value.executionSessionId !== "string" ||
    value.executionSessionId.length === 0 ||
    !isGraphResourcePath(value.graphPath) ||
    !isPositiveDecimalId(value.runId)
  )
    return fail("graph run identity");

  return {
    executionSessionId: value.executionSessionId,
    graphPath: value.graphPath,
    runId: value.runId,
  };
}

function parseResultInspectionSource(value: unknown): ResultInspectionSource {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, ["graphPath", "nodeId", "portAddress"]) ||
    !isGraphResourcePath(value.graphPath) ||
    !(value.nodeId === null || isUuid(value.nodeId)) ||
    !(value.portAddress === null || typeof value.portAddress === "string")
  ) {
    return fail("result inspection source");
  }
  return {
    graphPath: value.graphPath,
    nodeId: value.nodeId,
    portAddress: value.portAddress,
  };
}

function parseRunEventKind(value: unknown): RunEventKind {
  if (!isRecord(value)) return fail("run event kind");
  const type = parseDiscriminant(value.type, RUN_EVENT_KIND_TYPES, "run event kind variant");
  switch (type) {
    case "runStarted":
      if (!hasExactKeys(value, ["type", "outputs"]) || !Array.isArray(value.outputs))
        return fail("runStarted");
      return { type: "runStarted", outputs: value.outputs.map(parseGraphOutputRefDto) };
    case "runCompleted":
      if (!hasExactKeys(value, ["type"])) return fail("runCompleted");
      return { type: "runCompleted" };
    case "runErrored":
      if (!hasExactKeys(value, ["type", "code", "phase", "source"])) return fail("runErrored");
      return { type: "runErrored", ...parseErrorOutcome(value) };
    case "runCancelled":
      if (!hasExactKeys(value, ["type"])) return fail("runCancelled");
      return { type: "runCancelled" };
    case "resultInspectionRequested":
      if (
        !hasExactKeys(value, ["type", "resultId", "source"]) ||
        !isPositiveDecimalId(value.resultId)
      ) {
        return fail("resultInspectionRequested");
      }
      return {
        type: "resultInspectionRequested",
        resultId: value.resultId,
        source: parseResultInspectionSource(value.source),
      };
    default:
      return assertNever(type);
  }
}

export function parseRunEvent(value: unknown): RunEvent {
  if (!isRecord(value) || !hasExactKeys(value, ["run", "kind"])) {
    return fail("run event");
  }
  return {
    run: parseGraphRunIdentityDto(value.run),
    kind: parseRunEventKind(value.kind),
  };
}
