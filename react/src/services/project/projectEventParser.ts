import type { ResourceMutationResultDto } from "@/shared/types/dto/editorMutation";
import type {
  LifecycleMutationResultDto,
  ProjectActivationResult,
} from "@/shared/types/domain/project";
import { parseProjectActivationResult, parseLifecycleMutationResult } from "./projectWireParser";
import { parseResourceMutationResultDto } from "@/shared/types/dto/resourceMutationResultWireParser";

type UnknownRecord = Record<string, unknown>;

export interface ProjectLoadedPayload {
  readonly result: ProjectActivationResult;
}

export interface ProjectLifecycleCommittedPayload {
  readonly result: LifecycleMutationResultDto;
}

export interface ProjectIndexInvalidatedPayload {
  readonly projectInstanceId: string;
  readonly source: "watcher";
  readonly version: number;
}

export interface ResourceMutationCommittedPayload {
  readonly result: ResourceMutationResultDto;
}

export type ProjectEvent =
  | { readonly type: "ProjectLoaded"; readonly payload: ProjectLoadedPayload }
  | { readonly type: "ProjectCleared"; readonly payload: { readonly projectInstanceId: string } }
  | {
      readonly type: "ProjectLifecycleCommitted";
      readonly payload: ProjectLifecycleCommittedPayload;
    }
  | {
      readonly type: "ProjectIndexInvalidated";
      readonly payload: ProjectIndexInvalidatedPayload;
    }
  | {
      readonly type: "ResourceMutationCommitted";
      readonly payload: ResourceMutationCommittedPayload;
    };

export type ProjectEventParseCode = "invalidEnvelope" | "unknownType" | "invalidPayload";

export type ProjectEventParseOutcome =
  | { readonly ok: true; readonly event: ProjectEvent }
  | { readonly ok: false; readonly code: ProjectEventParseCode };

const PROJECT_EVENT_TYPES = {
  ProjectLoaded: true,
  ProjectCleared: true,
  ProjectLifecycleCommitted: true,
  ProjectIndexInvalidated: true,
  ResourceMutationCommitted: true,
} as const;

type ProjectEventType = keyof typeof PROJECT_EVENT_TYPES;

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

function isWatcherVersion(value: unknown): value is number {
  return Number.isSafeInteger(value) && (value as number) >= 1;
}

function parseProjectLoadedPayload(value: unknown): ProjectLoadedPayload | null {
  if (!isRecord(value) || !hasExactKeys(value, ["result"])) return null;
  try {
    return { result: parseProjectActivationResult(value.result) };
  } catch {
    return null;
  }
}

function parseLifecyclePayload(value: unknown): ProjectLifecycleCommittedPayload | null {
  if (!isRecord(value) || !hasExactKeys(value, ["result"])) return null;
  try {
    return { result: parseLifecycleMutationResult(value.result) };
  } catch {
    return null;
  }
}

function parseIndexInvalidatedPayload(value: unknown): ProjectIndexInvalidatedPayload | null {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, ["projectInstanceId", "source", "version"]) ||
    !isNonEmptyString(value.projectInstanceId) ||
    value.source !== "watcher" ||
    !isWatcherVersion(value.version)
  ) {
    return null;
  }
  return {
    projectInstanceId: value.projectInstanceId,
    source: value.source,
    version: value.version,
  };
}

function parseResourceMutationPayload(value: unknown): ResourceMutationCommittedPayload | null {
  if (!isRecord(value) || !hasExactKeys(value, ["result"])) return null;
  try {
    return { result: parseResourceMutationResultDto(value.result) };
  } catch {
    return null;
  }
}

function isProjectEventType(value: unknown): value is ProjectEventType {
  return (
    typeof value === "string" && Object.prototype.hasOwnProperty.call(PROJECT_EVENT_TYPES, value)
  );
}

function invalidEnvelope(): ProjectEventParseOutcome {
  return { ok: false, code: "invalidEnvelope" };
}

function invalidPayload(): ProjectEventParseOutcome {
  return { ok: false, code: "invalidPayload" };
}

function unknownType(): ProjectEventParseOutcome {
  return { ok: false, code: "unknownType" };
}

export function parseProjectEvent(value: unknown): ProjectEventParseOutcome {
  if (!isRecord(value) || !hasExactKeys(value, ["type", "payload"])) return invalidEnvelope();
  if (value.type !== "Project" && value.type !== "Resource") return unknownType();
  if (!isRecord(value.payload) || !Object.prototype.hasOwnProperty.call(value.payload, "type")) {
    return invalidEnvelope();
  }

  const nested = value.payload;
  if (!isProjectEventType(nested.type)) return unknownType();
  if (!hasExactKeys(nested, ["type", "payload"])) return invalidEnvelope();

  const isResourceEvent = nested.type === "ProjectIndexInvalidated";
  if (
    (isResourceEvent && value.type !== "Resource") ||
    (!isResourceEvent && value.type !== "Project")
  ) {
    return invalidEnvelope();
  }

  switch (nested.type) {
    case "ProjectCleared": {
      const payload = nested.payload;
      if (
        !isRecord(payload) ||
        !hasExactKeys(payload, ["projectInstanceId"]) ||
        typeof payload.projectInstanceId !== "string" ||
        !payload.projectInstanceId.trim()
      ) {
        return invalidPayload();
      }
      return {
        ok: true,
        event: { type: nested.type, payload: { projectInstanceId: payload.projectInstanceId } },
      };
    }
    case "ProjectLoaded": {
      const payload = parseProjectLoadedPayload(nested.payload);
      return payload === null
        ? invalidPayload()
        : { ok: true, event: { type: nested.type, payload } };
    }
    case "ProjectLifecycleCommitted": {
      const payload = parseLifecyclePayload(nested.payload);
      return payload === null
        ? invalidPayload()
        : { ok: true, event: { type: nested.type, payload } };
    }
    case "ProjectIndexInvalidated": {
      const payload = parseIndexInvalidatedPayload(nested.payload);
      return payload === null
        ? invalidPayload()
        : { ok: true, event: { type: nested.type, payload } };
    }
    case "ResourceMutationCommitted": {
      const payload = parseResourceMutationPayload(nested.payload);
      return payload === null
        ? invalidPayload()
        : { ok: true, event: { type: nested.type, payload } };
    }
  }
}
