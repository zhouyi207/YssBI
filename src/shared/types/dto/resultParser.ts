import { isGraphResourcePath, isPortAddressDto, isUuid } from "./editorProjectionGuards";
import { isResultPlotKind } from "./result";
import type {
  GraphOutputRefDto,
  PinResultEntry,
  ResultDataSeriesMetadata,
  ResultDescriptor,
  ResultFailure,
  ResultPage,
  ResultPresentation,
  ResultProvenance,
  ResultReportKind,
  ResultState,
  ResultUsage,
  ResultValue,
  ResultValueKind,
} from "./result";

type UnknownRecord = Record<string, unknown>;

const DECIMAL_ID_PATTERN = /^(0|[1-9]\d*)$/;
const REPORT_KINDS = new Set([
  "olsSummary",
  "binarySummary",
  "iv2slsSummary",
  "ivLimlSummary",
  "praisSummary",
  "varSummary",
  "varSoc",
  "panelSummary",
  "panelDid",
  "dfAdfSummary",
  "dfAdfSummaryList",
  "vecSummary",
  "vecRankSummary",
]);
const VALUE_KINDS = new Set(["scalar", "sequence", "dataSeries", "unknown"]);
const ELEMENT_TYPES = new Set([
  "int64",
  "float64",
  "string",
  "boolean",
  "date",
  "datetime",
  "categorical",
]);

function fail(contract: string): never {
  throw new Error(`Invalid ${contract}`);
}

function isRecord(value: unknown): value is UnknownRecord {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function hasExactKeys(value: UnknownRecord, keys: readonly string[]): boolean {
  return (
    Object.keys(value).length === keys.length &&
    keys.every((key) => Object.prototype.hasOwnProperty.call(value, key))
  );
}

function isDecimalId(value: unknown): value is string {
  return typeof value === "string" && DECIMAL_ID_PATTERN.test(value);
}

function isNonNegativeInteger(value: unknown): value is number {
  return Number.isSafeInteger(value) && (value as number) >= 0;
}

function parseGraphOutput(value: unknown): GraphOutputRefDto {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, ["graphPath", "port"]) ||
    !isGraphResourcePath(value.graphPath) ||
    !isPortAddressDto(value.port)
  )
    return fail("result output");
  return { graphPath: value.graphPath, port: value.port };
}

export function parseResultPresentation(value: unknown): ResultPresentation {
  if (!isRecord(value) || typeof value.kind !== "string") return fail("result presentation");
  switch (value.kind) {
    case "inspector":
      if (!hasExactKeys(value, ["kind"])) return fail("inspector presentation");
      return { kind: "inspector" };
    case "plot":
      if (!hasExactKeys(value, ["kind", "chart"]) || !isResultPlotKind(value.chart))
        return fail("plot presentation");
      return { kind: "plot", chart: value.chart };
    case "report":
      if (
        !hasExactKeys(value, ["kind", "report"]) ||
        typeof value.report !== "string" ||
        !REPORT_KINDS.has(value.report)
      )
        return fail("report presentation");
      return { kind: "report", report: value.report as ResultReportKind };
    default:
      return fail("result presentation kind");
  }
}

export function parseResultState(value: unknown): ResultState {
  if (!isRecord(value) || typeof value.kind !== "string") return fail("result state");
  switch (value.kind) {
    case "pending": {
      if (
        !hasExactKeys(value, ["kind", "progress"]) ||
        !isRecord(value.progress) ||
        !hasExactKeys(value.progress, ["completed", "total"]) ||
        !isDecimalId(value.progress.completed) ||
        !(value.progress.total === null || isDecimalId(value.progress.total))
      ) {
        return fail("pending result state");
      }
      return {
        kind: "pending",
        progress: { completed: value.progress.completed, total: value.progress.total },
      };
    }
    case "ready":
      if (!hasExactKeys(value, ["kind"])) return fail("ready result state");
      return { kind: "ready" };
    case "failed":
      if (!hasExactKeys(value, ["kind", "failure"])) return fail("failed result state");
      return { kind: "failed", failure: parseResultFailure(value.failure) };
    case "cancelled":
      if (!hasExactKeys(value, ["kind"])) return fail("cancelled result state");
      return { kind: "cancelled" };
    default:
      return fail("result state kind");
  }
}

function parseResultFailure(value: unknown): ResultFailure {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, ["code", "cause", "upstreamResultIds"]) ||
    (value.code !== "execution_failed" && value.code !== "upstream_failed") ||
    !Array.isArray(value.upstreamResultIds) ||
    !value.upstreamResultIds.every(isDecimalId) ||
    !isRecord(value.cause) ||
    typeof value.cause.kind !== "string"
  )
    return fail("result failure");

  if (value.cause.kind === "execution") {
    if (!hasExactKeys(value.cause, ["kind"])) return fail("execution failure cause");
    return { ...value, cause: { kind: "execution" } } as ResultFailure;
  }
  if (value.cause.kind === "upstream") {
    if (
      !hasExactKeys(value.cause, ["kind", "upstreamResultId"]) ||
      !isDecimalId(value.cause.upstreamResultId)
    )
      return fail("upstream failure cause");
    return {
      code: value.code,
      cause: { kind: "upstream", upstreamResultId: value.cause.upstreamResultId },
      upstreamResultIds: value.upstreamResultIds,
    };
  }
  return fail("result failure cause");
}

function parseResultProvenance(value: unknown): ResultProvenance {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, [
      "runId",
      "activationId",
      "graphPath",
      "graphRevision",
      "nodeId",
      "output",
      "createdAtMs",
    ]) ||
    !isDecimalId(value.runId) ||
    !isDecimalId(value.activationId) ||
    !isGraphResourcePath(value.graphPath) ||
    !isDecimalId(value.graphRevision) ||
    !isUuid(value.nodeId) ||
    !isDecimalId(value.createdAtMs)
  )
    return fail("result provenance");
  return {
    runId: value.runId,
    activationId: value.activationId,
    graphPath: value.graphPath,
    graphRevision: value.graphRevision,
    nodeId: value.nodeId,
    output: value.output === null ? null : parseGraphOutput(value.output),
    createdAtMs: value.createdAtMs,
  };
}

function parseMetadata(value: unknown): ResultDataSeriesMetadata | null {
  if (value === null) return null;
  if (
    !isRecord(value) ||
    !hasExactKeys(value, ["elementType", "length", "nullCount", "name", "format"]) ||
    typeof value.elementType !== "string" ||
    !ELEMENT_TYPES.has(value.elementType) ||
    !isNonNegativeInteger(value.length) ||
    !isNonNegativeInteger(value.nullCount) ||
    !(value.name === null || typeof value.name === "string") ||
    !(value.format === null || typeof value.format === "string")
  )
    return fail("result metadata");
  return value as unknown as ResultDataSeriesMetadata;
}

function parseValueKind(value: unknown, allowUnknown = true): ResultValueKind {
  if (
    typeof value !== "string" ||
    !VALUE_KINDS.has(value) ||
    (!allowUnknown && value === "unknown")
  ) {
    return fail("result value kind");
  }
  return value as ResultValueKind;
}

export function parseResultDescriptor(value: unknown): ResultDescriptor {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, [
      "resultId",
      "state",
      "provenance",
      "presentation",
      "valueKind",
      "metadata",
      "totalCount",
      "title",
    ]) ||
    !isDecimalId(value.resultId) ||
    !(value.totalCount === null || isNonNegativeInteger(value.totalCount)) ||
    typeof value.title !== "string"
  )
    return fail("result descriptor");
  return {
    resultId: value.resultId,
    state: parseResultState(value.state),
    provenance: parseResultProvenance(value.provenance),
    presentation: parseResultPresentation(value.presentation),
    valueKind: parseValueKind(value.valueKind),
    metadata: parseMetadata(value.metadata),
    totalCount: value.totalCount,
    title: value.title,
  };
}

export function parseResultValue(value: unknown): ResultValue {
  if (!isRecord(value) || !hasExactKeys(value, ["kind", "value"])) return fail("result value");
  if (value.kind === "value") return { kind: "value", value: value.value };
  if ((value.kind === "sequence" || value.kind === "dataSeries") && Array.isArray(value.value)) {
    return { kind: value.kind, value: value.value };
  }
  return fail("result value kind");
}

export function parseResultPage(value: unknown): ResultPage {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, [
      "resultId",
      "offset",
      "requestedLimit",
      "actualCount",
      "totalCount",
      "hasMore",
      "nextOffset",
      "valueKind",
      "metadata",
      "values",
    ]) ||
    !isDecimalId(value.resultId) ||
    !isNonNegativeInteger(value.offset) ||
    !isNonNegativeInteger(value.requestedLimit) ||
    !isNonNegativeInteger(value.actualCount) ||
    !isNonNegativeInteger(value.totalCount) ||
    typeof value.hasMore !== "boolean" ||
    !(value.nextOffset === null || isNonNegativeInteger(value.nextOffset)) ||
    !Array.isArray(value.values)
  )
    return fail("result page");
  return {
    resultId: value.resultId,
    offset: value.offset,
    requestedLimit: value.requestedLimit,
    actualCount: value.actualCount,
    totalCount: value.totalCount,
    hasMore: value.hasMore,
    nextOffset: value.nextOffset,
    valueKind: parseValueKind(value.valueKind, false) as ResultPage["valueKind"],
    metadata: parseMetadata(value.metadata),
    values: value.values,
  };
}

function parseUsage(value: unknown): ResultUsage {
  if (!isRecord(value) || typeof value.kind !== "string") return fail("result usage");
  if (value.kind === "produced" && hasExactKeys(value, ["kind"])) return { kind: "produced" };
  if (
    value.kind === "reused" &&
    hasExactKeys(value, ["kind", "originalActivationId"]) &&
    isDecimalId(value.originalActivationId)
  ) {
    return { kind: "reused", originalActivationId: value.originalActivationId };
  }
  return fail("result usage variant");
}

export function parsePinResultEntry(value: unknown): PinResultEntry {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, [
      "resultId",
      "runId",
      "activationId",
      "graphRevision",
      "createdAtMs",
      "usage",
      "state",
    ]) ||
    !isDecimalId(value.resultId) ||
    !isDecimalId(value.runId) ||
    !isDecimalId(value.activationId) ||
    !isDecimalId(value.graphRevision) ||
    !isDecimalId(value.createdAtMs)
  )
    return fail("pin result entry");
  return {
    resultId: value.resultId,
    runId: value.runId,
    activationId: value.activationId,
    graphRevision: value.graphRevision,
    createdAtMs: value.createdAtMs,
    usage: parseUsage(value.usage),
    state: parseResultState(value.state),
  };
}

export function parsePinResultHistory(value: unknown): PinResultEntry[] {
  if (!Array.isArray(value)) return fail("pin result history");
  return value.map(parsePinResultEntry);
}
