import {
  DIAGNOSTIC_DOMAINS,
  DIAGNOSTIC_LEVELS,
  DIAGNOSTIC_ORIGINS,
  type DiagnosticBatchDto,
  type DiagnosticFieldValueDto,
  type DiagnosticDomain,
  type DiagnosticFieldsDto,
  type DiagnosticLevel,
  type DiagnosticOrigin,
  type DiagnosticRecordDto,
  type DiagnosticSubscriptionDto,
} from "./diagnostics";

type UnknownRecord = Record<string, unknown>;

const DIAGNOSTIC_LEVEL_SET = new Set<string>(DIAGNOSTIC_LEVELS);
const DIAGNOSTIC_ORIGIN_SET = new Set<string>(DIAGNOSTIC_ORIGINS);
const DIAGNOSTIC_DOMAIN_SET = new Set<string>(DIAGNOSTIC_DOMAINS);
const RECORD_REQUIRED_KEYS = [
  "streamId",
  "sequence",
  "timestamp",
  "level",
  "origin",
  "domain",
  "target",
  "message",
  "fields",
] as const;
const RECORD_OPTIONAL_KEYS = ["event", "source"] as const;

function fail(contract: string): never {
  throw new Error(`Invalid ${contract}`);
}

function isRecord(value: unknown): value is UnknownRecord {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function hasExactContractKeys(
  value: UnknownRecord,
  required: readonly string[],
  optional: readonly string[] = [],
): boolean {
  const allowed = new Set([...required, ...optional]);
  const keys = Object.keys(value);
  return (
    required.every((key) => Object.prototype.hasOwnProperty.call(value, key)) &&
    keys.every((key) => allowed.has(key))
  );
}

function isNonEmptyString(value: unknown): value is string {
  return typeof value === "string" && value.trim().length > 0;
}

function isSequence(value: unknown): value is number {
  return Number.isSafeInteger(value) && (value as number) >= 0;
}

function isDiagnosticLevel(value: unknown): value is DiagnosticLevel {
  return typeof value === "string" && DIAGNOSTIC_LEVEL_SET.has(value);
}

function isDiagnosticOrigin(value: unknown): value is DiagnosticOrigin {
  return typeof value === "string" && DIAGNOSTIC_ORIGIN_SET.has(value);
}

function isDiagnosticDomain(value: unknown): value is DiagnosticDomain {
  return typeof value === "string" && DIAGNOSTIC_DOMAIN_SET.has(value);
}

function isFieldValue(value: unknown): value is DiagnosticFieldValueDto {
  if (value === null || typeof value === "string" || typeof value === "boolean") return true;
  if (typeof value === "number") return Number.isFinite(value);
  if (Array.isArray(value)) return value.every(isFieldValue);
  return isRecord(value) && Object.values(value).every(isFieldValue);
}

function parseFields(value: unknown): DiagnosticFieldsDto {
  if (!isRecord(value) || !Object.values(value).every(isFieldValue)) {
    return fail("diagnostic fields");
  }
  return value as DiagnosticFieldsDto;
}

function parseOptionalString(value: UnknownRecord, key: "event" | "source"): string | undefined {
  if (!Object.prototype.hasOwnProperty.call(value, key)) return undefined;
  return typeof value[key] === "string" ? value[key] : fail(`diagnostic ${key}`);
}

export function parseDiagnosticRecordDto(value: unknown): DiagnosticRecordDto {
  if (
    !isRecord(value) ||
    !hasExactContractKeys(value, RECORD_REQUIRED_KEYS, RECORD_OPTIONAL_KEYS) ||
    !isNonEmptyString(value.streamId) ||
    !isSequence(value.sequence) ||
    !isNonEmptyString(value.timestamp) ||
    !isDiagnosticLevel(value.level) ||
    !isDiagnosticOrigin(value.origin) ||
    !isDiagnosticDomain(value.domain) ||
    !isNonEmptyString(value.target) ||
    typeof value.message !== "string"
  ) {
    return fail("diagnostic record");
  }

  const event = parseOptionalString(value, "event");
  const source = parseOptionalString(value, "source");
  return {
    streamId: value.streamId,
    sequence: value.sequence,
    timestamp: value.timestamp,
    level: value.level,
    origin: value.origin,
    domain: value.domain,
    target: value.target,
    ...(event === undefined ? {} : { event }),
    message: value.message,
    ...(source === undefined ? {} : { source }),
    fields: parseFields(value.fields),
  };
}

function parseEntries(value: unknown, streamId: string, contract: string): DiagnosticRecordDto[] {
  if (!Array.isArray(value)) return fail(contract);
  const entries = value.map(parseDiagnosticRecordDto);
  if (entries.some((entry) => entry.streamId !== streamId)) return fail(contract);
  return entries;
}

export function parseDiagnosticSubscriptionDto(value: unknown): DiagnosticSubscriptionDto {
  const keys = ["subscriptionId", "streamId", "entries", "latestSequence", "truncated"] as const;
  if (
    !isRecord(value) ||
    !hasExactContractKeys(value, keys) ||
    !isNonEmptyString(value.subscriptionId) ||
    !isNonEmptyString(value.streamId) ||
    !isSequence(value.latestSequence) ||
    typeof value.truncated !== "boolean"
  ) {
    return fail("diagnostic subscription");
  }

  const latestSequence = value.latestSequence as number;
  const entries = parseEntries(value.entries, value.streamId, "diagnostic subscription entries");
  if (entries.some((entry) => entry.sequence > latestSequence)) {
    return fail("diagnostic subscription sequence");
  }
  return {
    subscriptionId: value.subscriptionId,
    streamId: value.streamId,
    entries,
    latestSequence,
    truncated: value.truncated,
  };
}

export function parseDiagnosticBatchDto(value: unknown): DiagnosticBatchDto {
  const keys = ["streamId", "entries"] as const;
  if (!isRecord(value) || !hasExactContractKeys(value, keys) || !isNonEmptyString(value.streamId)) {
    return fail("diagnostic batch");
  }
  return {
    streamId: value.streamId,
    entries: parseEntries(value.entries, value.streamId, "diagnostic batch entries"),
  };
}
