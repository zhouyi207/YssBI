import { z } from "zod";
import type {
  LifecycleMutationResultDto,
  ProjectActivationResult,
  ProjectRecordRow,
} from "@/shared/types/domain/project";

type UnknownRecord = Record<string, unknown>;

function isRecord(value: unknown): value is UnknownRecord {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function hasExactKeys(value: UnknownRecord, keys: readonly string[]): boolean {
  return (
    Object.keys(value).length === keys.length &&
    keys.every((key) => Object.prototype.hasOwnProperty.call(value, key))
  );
}

function isNonEmptyString(value: unknown): value is string {
  return typeof value === "string" && value.length > 0;
}

function isNullableNonEmptyString(value: unknown): value is string | null {
  return value === null || isNonEmptyString(value);
}

function isRevision(value: unknown): value is number {
  return Number.isSafeInteger(value) && (value as number) >= 0;
}

export function parseProjectActivationResult(value: unknown): ProjectActivationResult {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, ["path", "projectInstanceId", "activationRevision"]) ||
    !isNonEmptyString(value.path) ||
    !isNonEmptyString(value.projectInstanceId) ||
    !isRevision(value.activationRevision)
  )
    throw new TypeError("Invalid project activation");
  return {
    path: value.path,
    projectInstanceId: value.projectInstanceId,
    activationRevision: value.activationRevision,
  };
}

export function parseProjectRecord(value: unknown): ProjectRecordRow {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, [
      "id",
      "name",
      "path",
      "createdAt",
      "lastOpenedAt",
      "isFavorite",
      "rootIdentity",
      "rootIdentityState",
    ]) ||
    !isNonEmptyString(value.id) ||
    !isNonEmptyString(value.name) ||
    !isNonEmptyString(value.path) ||
    !isNonEmptyString(value.createdAt) ||
    !isNullableNonEmptyString(value.lastOpenedAt) ||
    typeof value.isFavorite !== "boolean" ||
    typeof value.rootIdentity !== "string" ||
    (value.rootIdentityState !== "valid" && value.rootIdentityState !== "invalid")
  ) {
    throw new TypeError("Invalid project record");
  }
  return {
    id: value.id,
    name: value.name,
    path: value.path,
    createdAt: value.createdAt,
    lastOpenedAt: value.lastOpenedAt,
    isFavorite: value.isFavorite,
    rootIdentity: value.rootIdentity,
    rootIdentityState: value.rootIdentityState,
  };
}

function parseLifecycleRecovery(
  value: unknown,
): NonNullable<LifecycleMutationResultDto["recovery"]> {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, ["required", "action", "path", "identity"]) ||
    typeof value.required !== "boolean" ||
    !isNonEmptyString(value.action) ||
    !isNullableNonEmptyString(value.path) ||
    !isNullableNonEmptyString(value.identity)
  ) {
    throw new TypeError("Invalid project lifecycle recovery");
  }
  return {
    required: value.required,
    action: value.action,
    path: value.path,
    identity: value.identity,
  };
}

export function parseLifecycleMutationResult(result: unknown): LifecycleMutationResultDto {
  if (!isRecord(result)) throw new TypeError("Invalid project lifecycle result");
  if (
    !hasExactKeys(result, [
      "operationId",
      "kind",
      "oldProjectInstanceId",
      "newProjectInstanceId",
      "phase",
      "outcome",
      "record",
      "path",
      "recovery",
      "invalidation",
    ]) ||
    !isNonEmptyString(result.operationId) ||
    (result.kind !== "saveAs" &&
      result.kind !== "create" &&
      result.kind !== "delete" &&
      result.kind !== "registryCleanup" &&
      result.kind !== "load" &&
      result.kind !== "clear") ||
    !isNullableNonEmptyString(result.oldProjectInstanceId) ||
    !isNullableNonEmptyString(result.newProjectInstanceId) ||
    (result.phase !== "destinationCommitted" &&
      result.phase !== "registryCommitted" &&
      result.phase !== "authorityCommitted") ||
    (result.outcome !== "committed" &&
      result.outcome !== "registryFailed" &&
      result.outcome !== "activationFailed" &&
      result.outcome !== "registryPending") ||
    !isNullableNonEmptyString(result.path) ||
    !isRecord(result.invalidation) ||
    !hasExactKeys(result.invalidation, ["project", "registry"]) ||
    typeof result.invalidation.project !== "boolean" ||
    typeof result.invalidation.registry !== "boolean"
  ) {
    throw new TypeError("Invalid project lifecycle result");
  }

  const record = result.record === null ? null : parseProjectRecord(result.record);
  const recovery = result.recovery === null ? null : parseLifecycleRecovery(result.recovery);

  return {
    operationId: result.operationId,
    kind: result.kind,
    oldProjectInstanceId: result.oldProjectInstanceId,
    newProjectInstanceId: result.newProjectInstanceId,
    phase: result.phase,
    outcome: result.outcome,
    record,
    path: result.path,
    recovery,
    invalidation: {
      project: result.invalidation.project,
      registry: result.invalidation.registry,
    },
  };
}

const count = z.number().int().nonnegative();
export const projectPathSchema = z.string().min(1);
export const nullableProjectPathSchema = projectPathSchema.nullable();
export const projectFlagSchema = z.boolean();
const projectRecordsSchema = z.array(z.unknown());
export function parseProjectRecords(value: unknown): ProjectRecordRow[] {
  return projectRecordsSchema.parse(value).map(parseProjectRecord);
}
const scanResultSchema = z
  .strictObject({
    discovered: count,
    newlyRegistered: count,
    projects: projectRecordsSchema,
  })
  .refine(
    (value) =>
      value.projects.length === value.discovered && value.newlyRegistered <= value.discovered,
    "Invalid project scan counts",
  );
export function parseScanProjectsResult(value: unknown) {
  const result = scanResultSchema.parse(value);
  return { ...result, projects: result.projects.map(parseProjectRecord) };
}
export const projectCleanupResultSchema = z.strictObject({ removed: count });
export const projectScanProgressSchema = z
  .discriminatedUnion("kind", [
    z.strictObject({ kind: z.literal("scanning") }),
    z.strictObject({ kind: z.literal("discovered"), count }),
    z.strictObject({ kind: z.literal("registering"), current: count, total: count }),
  ])
  .refine(
    (value) => value.kind !== "registering" || value.current <= value.total,
    "Invalid project scan progress",
  );
export const projectCleanupProgressSchema = z
  .discriminatedUnion("kind", [
    z.strictObject({ kind: z.literal("checking"), current: count, total: count }),
    z.strictObject({ kind: z.literal("removing"), removed: count, total: count }),
  ])
  .refine(
    (value) => (value.kind === "checking" ? value.current : value.removed) <= value.total,
    "Invalid project cleanup progress",
  );
export type ProjectScanProgressEvent = z.infer<typeof projectScanProgressSchema>;
export type ProjectCleanupProgressEvent = z.infer<typeof projectCleanupProgressSchema>;
