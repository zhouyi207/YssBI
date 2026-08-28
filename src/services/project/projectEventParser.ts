import type {
  GraphDeltaDto,
  HistoryStatusDto,
  ResourceKeyDto,
  ResourceMutationResultDto,
} from '@/shared/types/dto/editorMutation';
import {
  parseGraphDeltaDto,
  parseHistoryStatusDto,
} from '@/shared/types/dto/editorMutationWireParser';
import type {
  ComputationSettingsMutationReceiptDto,
  ProjectComputationSettingsDto,
} from '@/shared/types/dto/projectComputationSettings';
import {
  parseComputationSettingsMutationReceipt,
} from '@/shared/types/dto/projectComputationSettings';
import type {
  LifecycleMutationResultDto,
  ProjectRecordRow,
  ProjectSaveResultDto,
} from '@/shared/types/dto/project';
import { parseResourceMutationResultDto } from '@/shared/types/dto/resourceMutationResultWireParser';

type UnknownRecord = Record<string, unknown>;

export interface ProjectLoadedPayload {
  readonly result: {
    readonly path: string;
    readonly projectInstanceId: string;
    readonly activationRevision: number;
  };
}

export interface ProjectLifecycleCommittedPayload {
  readonly result: LifecycleMutationResultDto;
}

export interface ProjectSavedPayload {
  readonly result: ProjectSaveResultDto;
}

export interface ComputationSettingsChangedPayload {
  readonly result: ComputationSettingsMutationReceiptDto;
}

export interface ProjectIndexInvalidatedPayload {
  readonly projectInstanceId: string;
  readonly source: 'watcher';
  readonly version: number;
}

export interface GraphDeltaEventPayload {
  readonly projectInstanceId: string;
  readonly delta: GraphDeltaDto;
}

export interface ResourceMutationCommittedPayload {
  readonly result: ResourceMutationResultDto;
}

export type ProjectEvent =
  | { readonly type: 'ProjectLoaded'; readonly payload: ProjectLoadedPayload }
  | { readonly type: 'ProjectCleared'; readonly payload: undefined }
  | {
      readonly type: 'ProjectLifecycleCommitted';
      readonly payload: ProjectLifecycleCommittedPayload;
    }
  | { readonly type: 'ProjectSaved'; readonly payload: ProjectSavedPayload }
  | {
      readonly type: 'ComputationSettingsChanged';
      readonly payload: ComputationSettingsChangedPayload;
    }
  | {
      readonly type: 'ProjectIndexInvalidated';
      readonly payload: ProjectIndexInvalidatedPayload;
    }
  | { readonly type: 'GraphDelta'; readonly payload: GraphDeltaEventPayload }
  | {
      readonly type: 'ResourceMutationCommitted';
      readonly payload: ResourceMutationCommittedPayload;
    };

export type ProjectEventParseCode = 'invalidEnvelope' | 'unknownType' | 'invalidPayload';

export type ProjectEventParseOutcome =
  | { readonly ok: true; readonly event: ProjectEvent }
  | { readonly ok: false; readonly code: ProjectEventParseCode };

const PROJECT_EVENT_TYPES = {
  ProjectLoaded: true,
  ProjectCleared: true,
  ProjectLifecycleCommitted: true,
  ProjectSaved: true,
  ComputationSettingsChanged: true,
  ProjectIndexInvalidated: true,
  GraphDelta: true,
  ResourceMutationCommitted: true,
} as const;

type ProjectEventType = keyof typeof PROJECT_EVENT_TYPES;

function isRecord(value: unknown): value is UnknownRecord {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function hasExactKeys(value: UnknownRecord, keys: readonly string[]): boolean {
  return Object.keys(value).length === keys.length
    && keys.every((key) => Object.prototype.hasOwnProperty.call(value, key));
}

function isNonEmptyString(value: unknown): value is string {
  return typeof value === 'string' && value.length > 0;
}

function isNullableNonEmptyString(value: unknown): value is string | null {
  return value === null || isNonEmptyString(value);
}

function isRevision(value: unknown): value is number {
  return Number.isSafeInteger(value) && (value as number) >= 0;
}

function isWatcherVersion(value: unknown): value is number {
  return Number.isSafeInteger(value) && (value as number) >= 1;
}

function parseHistory(value: unknown): HistoryStatusDto | null {
  try {
    const parsed = parseHistoryStatusDto(value);
    return { canUndo: parsed.canUndo, canRedo: parsed.canRedo };
  } catch {
    return null;
  }
}

function parseResourceKey(value: unknown): ResourceKeyDto | null {
  if (!isRecord(value)
    || !hasExactKeys(value, ['kind', 'key'])
    || !isNonEmptyString(value.key)) {
    return null;
  }
  if (value.kind !== 'graph'
    && value.kind !== 'function'
    && value.kind !== 'variable'
    && value.kind !== 'database'
    && value.kind !== 'worksheet') {
    return null;
  }
  return { kind: value.kind, key: value.key };
}

function parseProjectLoadedPayload(value: unknown): ProjectLoadedPayload | null {
  if (!isRecord(value) || !hasExactKeys(value, ['result']) || !isRecord(value.result)) {
    return null;
  }
  const result = value.result;
  if (!hasExactKeys(result, ['path', 'projectInstanceId', 'activationRevision'])
    || !isNonEmptyString(result.path)
    || !isNonEmptyString(result.projectInstanceId)
    || !isRevision(result.activationRevision)) {
    return null;
  }
  return {
    result: {
      path: result.path,
      projectInstanceId: result.projectInstanceId,
      activationRevision: result.activationRevision,
    },
  };
}

function parseProjectRecord(value: unknown): ProjectRecordRow | null {
  if (!isRecord(value)
    || !hasExactKeys(value, [
      'id', 'name', 'path', 'createdAt', 'lastOpenedAt', 'isFavorite', 'rootIdentity',
    ])
    || !isNonEmptyString(value.id)
    || !isNonEmptyString(value.name)
    || !isNonEmptyString(value.path)
    || !isNonEmptyString(value.createdAt)
    || !isNullableNonEmptyString(value.lastOpenedAt)
    || typeof value.isFavorite !== 'boolean'
    || !isNonEmptyString(value.rootIdentity)) {
    return null;
  }
  return {
    id: value.id,
    name: value.name,
    path: value.path,
    createdAt: value.createdAt,
    lastOpenedAt: value.lastOpenedAt,
    isFavorite: value.isFavorite,
    rootIdentity: value.rootIdentity,
  };
}

function parseLifecycleRecovery(value: unknown): LifecycleMutationResultDto['recovery'] | null {
  if (!isRecord(value)
    || !hasExactKeys(value, ['required', 'action', 'path', 'identity'])
    || typeof value.required !== 'boolean'
    || !isNonEmptyString(value.action)
    || !isNullableNonEmptyString(value.path)
    || !isNullableNonEmptyString(value.identity)) {
    return null;
  }
  return {
    required: value.required,
    action: value.action,
    path: value.path,
    identity: value.identity,
  };
}

function parseLifecyclePayload(value: unknown): ProjectLifecycleCommittedPayload | null {
  if (!isRecord(value) || !hasExactKeys(value, ['result']) || !isRecord(value.result)) {
    return null;
  }
  const result = value.result;
  if (!hasExactKeys(result, [
    'operationId', 'kind', 'oldProjectInstanceId', 'newProjectInstanceId', 'phase', 'outcome',
    'record', 'path', 'recovery', 'invalidation',
  ])
    || !isNonEmptyString(result.operationId)
    || (result.kind !== 'saveAs'
      && result.kind !== 'create'
      && result.kind !== 'delete'
      && result.kind !== 'registryCleanup'
      && result.kind !== 'load'
      && result.kind !== 'clear')
    || !isNullableNonEmptyString(result.oldProjectInstanceId)
    || !isNullableNonEmptyString(result.newProjectInstanceId)
    || (result.phase !== 'destinationCommitted'
      && result.phase !== 'registryCommitted'
      && result.phase !== 'authorityCommitted')
    || (result.outcome !== 'committed'
      && result.outcome !== 'registryFailed'
      && result.outcome !== 'activationFailed'
      && result.outcome !== 'registryPending')
    || !isNullableNonEmptyString(result.path)
    || !isRecord(result.invalidation)
    || !hasExactKeys(result.invalidation, ['project', 'registry'])
    || typeof result.invalidation.project !== 'boolean'
    || typeof result.invalidation.registry !== 'boolean') {
    return null;
  }

  const record = result.record === null ? null : parseProjectRecord(result.record);
  if (result.record !== null && record === null) return null;
  const recovery = result.recovery === null ? null : parseLifecycleRecovery(result.recovery);
  if (result.recovery !== null && recovery === null) return null;

  return {
    result: {
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
    },
  } as ProjectLifecycleCommittedPayload;
}

function parseProjectSavedPayload(value: unknown): ProjectSavedPayload | null {
  if (!isRecord(value) || !hasExactKeys(value, ['result']) || !isRecord(value.result)) {
    return null;
  }
  const result = value.result;
  if (!hasExactKeys(result, [
    'projectInstanceId', 'operationId', 'publicationRevision', 'affectedResources',
    'indexInvalidated', 'history',
  ])
    || !isNonEmptyString(result.projectInstanceId)
    || !isNonEmptyString(result.operationId)
    || !isRevision(result.publicationRevision)
    || !Array.isArray(result.affectedResources)
    || typeof result.indexInvalidated !== 'boolean') {
    return null;
  }

  const affectedResources = result.affectedResources.map(parseResourceKey);
  if (affectedResources.some((resource) => resource === null)) return null;
  const history = parseHistory(result.history);
  if (history === null) return null;

  return {
    result: {
      projectInstanceId: result.projectInstanceId,
      operationId: result.operationId,
      publicationRevision: result.publicationRevision,
      affectedResources: affectedResources as ResourceKeyDto[],
      indexInvalidated: result.indexInvalidated,
      history,
    },
  };
}

function parseSettingsPayload(value: unknown): ComputationSettingsChangedPayload | null {
  if (!isRecord(value) || !hasExactKeys(value, ['result'])) return null;
  try {
    const parsed = parseComputationSettingsMutationReceipt(value.result);
    const settings: ProjectComputationSettingsDto = {
      numeric: {
        tolerance: {
          absolute: parsed.settings.numeric.tolerance.absolute,
          relative: parsed.settings.numeric.tolerance.relative,
        },
      },
      missingValues: { statistics: parsed.settings.missingValues.statistics },
    };
    return {
      result: {
        projectInstanceId: parsed.projectInstanceId,
        operationId: parsed.operationId,
        settingsRevision: parsed.settingsRevision,
        publicationRevision: parsed.publicationRevision,
        settings,
      },
    };
  } catch {
    return null;
  }
}

function parseIndexInvalidatedPayload(value: unknown): ProjectIndexInvalidatedPayload | null {
  if (!isRecord(value)
    || !hasExactKeys(value, ['projectInstanceId', 'source', 'version'])
    || !isNonEmptyString(value.projectInstanceId)
    || value.source !== 'watcher'
    || !isWatcherVersion(value.version)) {
    return null;
  }
  return {
    projectInstanceId: value.projectInstanceId,
    source: value.source,
    version: value.version,
  };
}

function parseGraphDeltaPayload(value: unknown): GraphDeltaEventPayload | null {
  if (!isRecord(value)
    || !hasExactKeys(value, ['projectInstanceId', 'delta'])
    || !isNonEmptyString(value.projectInstanceId)) {
    return null;
  }
  try {
    return {
      projectInstanceId: value.projectInstanceId,
      delta: parseGraphDeltaDto(value.delta),
    };
  } catch {
    return null;
  }
}

function parseResourceMutationPayload(value: unknown): ResourceMutationCommittedPayload | null {
  if (!isRecord(value) || !hasExactKeys(value, ['result'])) return null;
  try {
    return { result: parseResourceMutationResultDto(value.result) };
  } catch {
    return null;
  }
}

function isProjectEventType(value: unknown): value is ProjectEventType {
  return typeof value === 'string'
    && Object.prototype.hasOwnProperty.call(PROJECT_EVENT_TYPES, value);
}

function invalidEnvelope(): ProjectEventParseOutcome {
  return { ok: false, code: 'invalidEnvelope' };
}

function invalidPayload(): ProjectEventParseOutcome {
  return { ok: false, code: 'invalidPayload' };
}

function unknownType(): ProjectEventParseOutcome {
  return { ok: false, code: 'unknownType' };
}

export function parseProjectEvent(value: unknown): ProjectEventParseOutcome {
  if (!isRecord(value) || !hasExactKeys(value, ['type', 'payload'])) return invalidEnvelope();
  if (value.type !== 'Project' && value.type !== 'Resource') return unknownType();
  if (!isRecord(value.payload)
    || !Object.prototype.hasOwnProperty.call(value.payload, 'type')) {
    return invalidEnvelope();
  }

  const nested = value.payload;
  if (!isProjectEventType(nested.type)) return unknownType();
  if (nested.type === 'ProjectCleared') {
    if (value.type !== 'Project' || !hasExactKeys(nested, ['type'])) return invalidEnvelope();
    return { ok: true, event: { type: 'ProjectCleared', payload: undefined } };
  }
  if (!hasExactKeys(nested, ['type', 'payload'])) return invalidEnvelope();

  const isResourceEvent = nested.type === 'ProjectIndexInvalidated';
  if ((isResourceEvent && value.type !== 'Resource')
    || (!isResourceEvent && value.type !== 'Project')) {
    return invalidEnvelope();
  }

  switch (nested.type) {
    case 'ProjectLoaded': {
      const payload = parseProjectLoadedPayload(nested.payload);
      return payload === null
        ? invalidPayload()
        : { ok: true, event: { type: nested.type, payload } };
    }
    case 'ProjectLifecycleCommitted': {
      const payload = parseLifecyclePayload(nested.payload);
      return payload === null
        ? invalidPayload()
        : { ok: true, event: { type: nested.type, payload } };
    }
    case 'ProjectSaved': {
      const payload = parseProjectSavedPayload(nested.payload);
      return payload === null
        ? invalidPayload()
        : { ok: true, event: { type: nested.type, payload } };
    }
    case 'ComputationSettingsChanged': {
      const payload = parseSettingsPayload(nested.payload);
      return payload === null
        ? invalidPayload()
        : { ok: true, event: { type: nested.type, payload } };
    }
    case 'ProjectIndexInvalidated': {
      const payload = parseIndexInvalidatedPayload(nested.payload);
      return payload === null
        ? invalidPayload()
        : { ok: true, event: { type: nested.type, payload } };
    }
    case 'GraphDelta': {
      const payload = parseGraphDeltaPayload(nested.payload);
      return payload === null
        ? invalidPayload()
        : { ok: true, event: { type: nested.type, payload } };
    }
    case 'ResourceMutationCommitted': {
      const payload = parseResourceMutationPayload(nested.payload);
      return payload === null
        ? invalidPayload()
        : { ok: true, event: { type: nested.type, payload } };
    }
  }
}
