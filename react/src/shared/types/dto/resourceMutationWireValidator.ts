import { hasValidResourceDeltaRevisions } from "@/shared/types/domain/resourceMutationValidation";
import { isMindPath } from "@/shared/types/domain/mind";
import { isDatabaseEngine } from "@/shared/types/domain/database";
import { isChartDocumentState } from "@/shared/types/domain/chart";
import { isDocPath } from "@/shared/types/domain/doc";
import type { ResourceDeltaDto } from "@/shared/types/dto/editorMutation";
import { isFunctionSignatureDto } from "@/shared/types/domain/editorMutation";
import { isGraphResourcePath, isUuid } from "@/shared/types/domain/editorProjectionGuards";

type UnknownRecord = Record<string, unknown>;

function isRecord(value: unknown): value is UnknownRecord {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function hasOwn(value: UnknownRecord, key: string): boolean {
  return Object.prototype.hasOwnProperty.call(value, key);
}

function hasExactKeys(value: UnknownRecord, keys: readonly string[]): boolean {
  return Object.keys(value).length === keys.length && keys.every((key) => hasOwn(value, key));
}

function isNullableString(value: unknown): value is string | null {
  return value === null || typeof value === "string";
}

function isFunctionPatch(value: unknown): boolean {
  return (
    isRecord(value) && isFunctionSignatureDto(value.before) && isFunctionSignatureDto(value.after)
  );
}

function isDatabaseDocument(value: unknown): value is UnknownRecord {
  return (
    isRecord(value) &&
    hasExactKeys(value, ["id", "engine", "schemaVersion", "required", "name"]) &&
    typeof value.id === "string" &&
    value.id.length > 0 &&
    isDatabaseEngine(value.engine) &&
    Number.isSafeInteger(value.schemaVersion) &&
    (value.schemaVersion as number) >= 0 &&
    typeof value.required === "boolean" &&
    isNullableString(value.name)
  );
}

function isDatabasePatch(value: unknown): boolean {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, ["before", "after"]) ||
    (value.before !== null && !isDatabaseDocument(value.before)) ||
    (value.after !== null && !isDatabaseDocument(value.after)) ||
    (value.before === null && value.after === null)
  )
    return false;
  return value.before === null || value.after === null || value.before.id === value.after.id;
}

function isNonEmptyPath(value: unknown): value is string {
  return typeof value === "string" && value.length > 0;
}

function isResourcePathMovePatch(
  value: unknown,
  graphPath: boolean,
): value is { from: string; to: string } {
  return (
    isRecord(value) &&
    hasExactKeys(value, ["from", "to"]) &&
    (graphPath ? isGraphResourcePath(value.from) : isNonEmptyPath(value.from)) &&
    (graphPath ? isGraphResourcePath(value.to) : isNonEmptyPath(value.to)) &&
    value.from !== value.to
  );
}

function isChartPatch(value: unknown): boolean {
  return (
    isRecord(value) &&
    hasExactKeys(value, ["before", "after"]) &&
    isChartDocumentState(value.before) &&
    isChartDocumentState(value.after)
  );
}

function isResourceLifecycleState(
  value: unknown,
  path: string,
  resourceKind: "graph" | "chart" | "mind" | "doc",
): boolean {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, ["revision", "path", "kind", "name"]) ||
    !Number.isSafeInteger(value.revision) ||
    (value.revision as number) < 0 ||
    value.path !== path ||
    typeof value.name !== "string" ||
    value.name.length === 0
  )
    return false;
  if (resourceKind === "mind") return value.kind === "mind" && isMindPath(path);
  if (resourceKind === "doc") return value.kind === "doc" && isDocPath(path);
  return resourceKind === "chart"
    ? value.kind === "chart"
    : value.kind === "event_graph" || value.kind === "function_graph";
}

function isResourceLifecyclePatch(
  value: unknown,
  path: string,
  resourceKind: "graph" | "chart" | "mind" | "doc",
): boolean {
  if (!isRecord(value) || !hasExactKeys(value, ["before", "after"])) return false;
  const beforeValid =
    value.before === null || isResourceLifecycleState(value.before, path, resourceKind);
  const afterValid =
    value.after === null || isResourceLifecycleState(value.after, path, resourceKind);
  return (
    beforeValid &&
    afterValid &&
    (resourceKind === "mind" || resourceKind === "doc"
      ? value.before !== null || value.after !== null
      : (value.before === null) !== (value.after === null))
  );
}

function isOperationCorrelation(value: unknown): value is string | null {
  return value === null || isUuid(value);
}

function isResourceAndPayload(value: UnknownRecord): boolean {
  if (
    !isRecord(value.resource) ||
    !hasExactKeys(value.resource, ["kind", "key"]) ||
    !isRecord(value.payload) ||
    !hasExactKeys(value.payload, ["kind", "patch"])
  )
    return false;
  const { kind, key } = value.resource;
  if (kind === "graph") {
    return (
      isGraphResourcePath(key) &&
      ((value.payload.kind === "resource_lifecycle" &&
        isResourceLifecyclePatch(value.payload.patch, key, "graph")) ||
        (value.payload.kind === "resource_move" &&
          isResourcePathMovePatch(value.payload.patch, true)))
    );
  }
  if (kind === "function") {
    return (
      isGraphResourcePath(key) &&
      value.payload.kind === "function" &&
      isFunctionPatch(value.payload.patch)
    );
  }
  if (kind === "mind" || kind === "doc") {
    const isPath = kind === "mind" ? isMindPath : isDocPath;
    if (!isPath(key)) return false;
    return (
      (value.payload.kind === "resource_lifecycle" &&
        isResourceLifecyclePatch(value.payload.patch, key, kind)) ||
      (value.payload.kind === "resource_move" &&
        isResourcePathMovePatch(value.payload.patch, false) &&
        isRecord(value.payload.patch) &&
        isPath(value.payload.patch.from) &&
        isPath(value.payload.patch.to) &&
        value.payload.patch.to === key)
    );
  }
  if (kind === "chart") {
    return (
      isNonEmptyPath(key) &&
      ((value.payload.kind === "chart" && isChartPatch(value.payload.patch)) ||
        (value.payload.kind === "resource_lifecycle" &&
          isResourceLifecyclePatch(value.payload.patch, key, "chart")) ||
        (value.payload.kind === "resource_move" &&
          isResourcePathMovePatch(value.payload.patch, false)))
    );
  }
  return (
    kind === "database" &&
    typeof key === "string" &&
    key.length > 0 &&
    value.payload.kind === "database" &&
    isDatabasePatch(value.payload.patch)
  );
}

function isResourceDelta(value: unknown): value is ResourceDeltaDto {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, ["resource", "fromRevision", "toRevision", "causedBy", "payload"]) ||
    !isResourceAndPayload(value)
  )
    return false;
  if (
    !Number.isSafeInteger(value.fromRevision) ||
    !Number.isSafeInteger(value.toRevision) ||
    (value.fromRevision as number) < 0 ||
    (value.toRevision as number) < 0 ||
    !isOperationCorrelation(value.causedBy)
  )
    return false;
  if (
    isRecord(value.payload) &&
    value.payload.kind === "resource_lifecycle" &&
    !isUuid(value.causedBy)
  )
    return false;
  return hasValidResourceDeltaRevisions(value as unknown as ResourceDeltaDto);
}

function deltaTarget(delta: ResourceDeltaDto): string {
  return `${delta.resource.kind}:${delta.resource.key}`;
}

export function areResourceDeltasValid(value: unknown): value is ResourceDeltaDto[] {
  if (!Array.isArray(value) || !value.every(isResourceDelta)) return false;
  const targets = new Set<string>();
  for (const delta of value) {
    const target = deltaTarget(delta);
    if (targets.has(target)) return false;
    targets.add(target);
  }
  return true;
}
