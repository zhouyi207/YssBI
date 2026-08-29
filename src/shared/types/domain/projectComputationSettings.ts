export const RECOMMENDED_PROJECT_COMPUTATION_SETTINGS = Object.freeze({
  numeric: Object.freeze({
    tolerance: Object.freeze({ absolute: 1e-12, relative: 1e-9 }),
  }),
  missingValues: Object.freeze({ statistics: 'listwise' as const }),
});

export type StatisticalMissingValuePolicy = 'listwise' | 'reject';

export interface ProjectComputationSettingsDto {
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

export interface ComputationSettingsSnapshotDto {
  projectInstanceId: string;
  settingsRevision: number;
  publicationRevision: number;
  settings: ProjectComputationSettingsDto;
}

export interface ComputationSettingsMutationRequestDto {
  projectInstanceId: string;
  operationId: string;
  expectedRevision: number;
  settings: ProjectComputationSettingsDto;
}

export interface ComputationSettingsMutationReceiptDto extends ComputationSettingsSnapshotDto {
  operationId: string;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function hasExactKeys(value: Record<string, unknown>, keys: readonly string[]): boolean {
  return Object.keys(value).length === keys.length
    && keys.every((key) => Object.prototype.hasOwnProperty.call(value, key));
}

function isRevision(value: unknown): value is number {
  return Number.isSafeInteger(value) && (value as number) >= 0;
}

export function parseProjectComputationSettings(value: unknown): ProjectComputationSettingsDto {
  if (!isRecord(value) || !hasExactKeys(value, ['numeric', 'missingValues'])) {
    throw new Error('Invalid project computation settings');
  }
  const numeric = value.numeric;
  const missingValues = value.missingValues;
  if (!isRecord(numeric) || !hasExactKeys(numeric, ['tolerance'])
    || !isRecord(numeric.tolerance)
    || !hasExactKeys(numeric.tolerance, ['absolute', 'relative'])
    || typeof numeric.tolerance.absolute !== 'number'
    || typeof numeric.tolerance.relative !== 'number'
    || !Number.isFinite(numeric.tolerance.absolute)
    || !Number.isFinite(numeric.tolerance.relative)
    || numeric.tolerance.absolute < 0
    || numeric.tolerance.relative < 0
    || (numeric.tolerance.absolute === 0 && numeric.tolerance.relative === 0)
    || !isRecord(missingValues)
    || !hasExactKeys(missingValues, ['statistics'])
    || (missingValues.statistics !== 'listwise' && missingValues.statistics !== 'reject')) {
    throw new Error('Invalid project computation settings');
  }
  return value as unknown as ProjectComputationSettingsDto;
}

function parseSnapshotBase(
  value: unknown,
  keys: readonly string[],
  errorMessage: string,
): ComputationSettingsSnapshotDto & Record<string, unknown> {
  if (!isRecord(value) || !hasExactKeys(value, keys)
    || typeof value.projectInstanceId !== 'string'
    || value.projectInstanceId.length === 0
    || !isRevision(value.settingsRevision)
    || !isRevision(value.publicationRevision)) {
    throw new Error(errorMessage);
  }
  try {
    parseProjectComputationSettings(value.settings);
  } catch {
    throw new Error(errorMessage);
  }
  return value as ComputationSettingsSnapshotDto & Record<string, unknown>;
}

export function parseComputationSettingsSnapshot(
  value: unknown,
): ComputationSettingsSnapshotDto {
  return parseSnapshotBase(
    value,
    ['projectInstanceId', 'settingsRevision', 'publicationRevision', 'settings'],
    'Invalid project computation settings response',
  );
}

export function parseComputationSettingsMutationReceipt(
  value: unknown,
): ComputationSettingsMutationReceiptDto {
  const parsed = parseSnapshotBase(
    value,
    ['projectInstanceId', 'operationId', 'settingsRevision', 'publicationRevision', 'settings'],
    'Invalid project computation settings receipt',
  );
  if (typeof parsed.operationId !== 'string' || parsed.operationId.length === 0) {
    throw new Error('Invalid project computation settings receipt');
  }
  return parsed as unknown as ComputationSettingsMutationReceiptDto;
}
