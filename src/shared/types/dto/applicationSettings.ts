/** Stable IPC contract for application-scoped backend settings. */

export const RECOMMENDED_COMPUTATION_SETTINGS = Object.freeze({
  numeric: Object.freeze({
    tolerance: Object.freeze({ absolute: 1e-12, relative: 1e-9 }),
  }),
  missingValues: Object.freeze({ statistics: "listwise" as const }),
});

export type StatisticalMissingValuePolicy = "listwise" | "reject";

export interface ComputationSettingsDto {
  numeric: {
    tolerance: {
      absolute: number;
      relative: number;
    };
  };
  missingValues: {
    statistics: StatisticalMissingValuePolicy;
  };
}

export interface ApplicationSettingsDto {
  computation: ComputationSettingsDto;
}

export interface ApplicationSettingsSnapshotDto {
  settingsRevision: number;
  settings: ApplicationSettingsDto;
}

export interface ApplicationSettingsMutationRequestDto {
  operationId: string;
  expectedRevision: number;
  settings: ApplicationSettingsDto;
}

export interface ApplicationSettingsMutationReceiptDto extends ApplicationSettingsSnapshotDto {
  operationId: string;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function hasExactKeys(value: Record<string, unknown>, keys: readonly string[]): boolean {
  return (
    Object.keys(value).length === keys.length &&
    keys.every((key) => Object.prototype.hasOwnProperty.call(value, key))
  );
}

function isRevision(value: unknown): value is number {
  return Number.isSafeInteger(value) && (value as number) >= 0;
}

export function parseComputationSettings(value: unknown): ComputationSettingsDto {
  if (!isRecord(value) || !hasExactKeys(value, ["numeric", "missingValues"])) {
    throw new Error("Invalid application computation settings");
  }
  const numeric = value.numeric;
  const missingValues = value.missingValues;
  if (
    !isRecord(numeric) ||
    !hasExactKeys(numeric, ["tolerance"]) ||
    !isRecord(numeric.tolerance) ||
    !hasExactKeys(numeric.tolerance, ["absolute", "relative"]) ||
    typeof numeric.tolerance.absolute !== "number" ||
    typeof numeric.tolerance.relative !== "number" ||
    !Number.isFinite(numeric.tolerance.absolute) ||
    !Number.isFinite(numeric.tolerance.relative) ||
    numeric.tolerance.absolute < 0 ||
    numeric.tolerance.relative < 0 ||
    (numeric.tolerance.absolute === 0 && numeric.tolerance.relative === 0) ||
    !isRecord(missingValues) ||
    !hasExactKeys(missingValues, ["statistics"]) ||
    (missingValues.statistics !== "listwise" && missingValues.statistics !== "reject")
  ) {
    throw new Error("Invalid application computation settings");
  }
  return value as unknown as ComputationSettingsDto;
}

function parseApplicationSettings(value: unknown): ApplicationSettingsDto {
  if (!isRecord(value) || !hasExactKeys(value, ["computation"])) {
    throw new Error("Invalid application settings");
  }
  parseComputationSettings(value.computation);
  return value as unknown as ApplicationSettingsDto;
}

function parseSnapshotBase(
  value: unknown,
  keys: readonly string[],
  errorMessage: string,
): ApplicationSettingsSnapshotDto & Record<string, unknown> {
  if (!isRecord(value) || !hasExactKeys(value, keys) || !isRevision(value.settingsRevision)) {
    throw new Error(errorMessage);
  }
  try {
    parseApplicationSettings(value.settings);
  } catch {
    throw new Error(errorMessage);
  }
  return value as ApplicationSettingsSnapshotDto & Record<string, unknown>;
}

export function parseApplicationSettingsSnapshot(value: unknown): ApplicationSettingsSnapshotDto {
  return parseSnapshotBase(
    value,
    ["settingsRevision", "settings"],
    "Invalid application settings response",
  );
}

export function parseApplicationSettingsMutationReceipt(
  value: unknown,
): ApplicationSettingsMutationReceiptDto {
  const parsed = parseSnapshotBase(
    value,
    ["operationId", "settingsRevision", "settings"],
    "Invalid application settings receipt",
  );
  if (typeof parsed.operationId !== "string" || parsed.operationId.length === 0) {
    throw new Error("Invalid application settings receipt");
  }
  return parsed as unknown as ApplicationSettingsMutationReceiptDto;
}
