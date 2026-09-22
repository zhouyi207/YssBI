import {
  LOG_DOMAINS,
  LOG_LEVELS,
  LOG_ORIGINS,
  type LogBatchDto,
  type LogFieldValueDto,
  type LogDomain,
  type LogFieldsDto,
  type LogLevel,
  type LogOrigin,
  type LogRecordDto,
  type LogSubscriptionDto,
} from "./log";

type UnknownRecord = Record<string, unknown>;

const LOG_LEVEL_SET = new Set<string>(LOG_LEVELS);
const LOG_ORIGIN_SET = new Set<string>(LOG_ORIGINS);
const LOG_DOMAIN_SET = new Set<string>(LOG_DOMAINS);
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

function isLogLevel(value: unknown): value is LogLevel {
  return typeof value === "string" && LOG_LEVEL_SET.has(value);
}

function isLogOrigin(value: unknown): value is LogOrigin {
  return typeof value === "string" && LOG_ORIGIN_SET.has(value);
}

function isLogDomain(value: unknown): value is LogDomain {
  return typeof value === "string" && LOG_DOMAIN_SET.has(value);
}

function isFieldValue(value: unknown): value is LogFieldValueDto {
  if (value === null || typeof value === "string" || typeof value === "boolean") return true;
  if (typeof value === "number") return Number.isFinite(value);
  if (Array.isArray(value)) return value.every(isFieldValue);
  return isRecord(value) && Object.values(value).every(isFieldValue);
}

function parseFields(value: unknown): LogFieldsDto {
  if (!isRecord(value) || !Object.values(value).every(isFieldValue)) {
    return fail("log fields");
  }
  return value as LogFieldsDto;
}

function parseOptionalString(value: UnknownRecord, key: "event" | "source"): string | undefined {
  if (!Object.prototype.hasOwnProperty.call(value, key)) return undefined;
  return typeof value[key] === "string" ? value[key] : fail(`log ${key}`);
}

export function parseLogRecordDto(value: unknown): LogRecordDto {
  if (
    !isRecord(value) ||
    !hasExactContractKeys(value, RECORD_REQUIRED_KEYS, RECORD_OPTIONAL_KEYS) ||
    !isNonEmptyString(value.streamId) ||
    !isSequence(value.sequence) ||
    !isNonEmptyString(value.timestamp) ||
    !isLogLevel(value.level) ||
    !isLogOrigin(value.origin) ||
    !isLogDomain(value.domain) ||
    !isNonEmptyString(value.target) ||
    typeof value.message !== "string"
  ) {
    return fail("log record");
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

function parseEntries(value: unknown, streamId: string, contract: string): LogRecordDto[] {
  if (!Array.isArray(value)) return fail(contract);
  const entries = value.map(parseLogRecordDto);
  if (entries.some((entry) => entry.streamId !== streamId)) return fail(contract);
  return entries;
}

export function parseLogSubscriptionDto(value: unknown): LogSubscriptionDto {
  const keys = ["subscriptionId", "streamId", "entries", "latestSequence", "truncated"] as const;
  if (
    !isRecord(value) ||
    !hasExactContractKeys(value, keys) ||
    !isNonEmptyString(value.subscriptionId) ||
    !isNonEmptyString(value.streamId) ||
    !isSequence(value.latestSequence) ||
    typeof value.truncated !== "boolean"
  ) {
    return fail("log subscription");
  }

  const latestSequence = value.latestSequence as number;
  const entries = parseEntries(value.entries, value.streamId, "log subscription entries");
  if (entries.some((entry) => entry.sequence > latestSequence)) {
    return fail("log subscription sequence");
  }
  return {
    subscriptionId: value.subscriptionId,
    streamId: value.streamId,
    entries,
    latestSequence,
    truncated: value.truncated,
  };
}

export function parseLogBatchDto(value: unknown): LogBatchDto {
  const keys = ["streamId", "entries"] as const;
  if (
    !isRecord(value) ||
    !hasExactContractKeys(value, keys, ["failure"]) ||
    !isNonEmptyString(value.streamId)
  ) {
    return fail("log batch");
  }
  const entries = parseEntries(value.entries, value.streamId, "log batch entries");
  if (
    Object.prototype.hasOwnProperty.call(value, "failure") &&
    (value.failure !== "storage_unavailable" || entries.length > 0)
  ) {
    return fail("log storage failure");
  }
  return {
    streamId: value.streamId,
    entries,
    ...(value.failure === "storage_unavailable" ? ({ failure: value.failure } as const) : {}),
  };
}
