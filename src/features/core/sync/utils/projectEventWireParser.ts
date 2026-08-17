import type {
  GraphDeltaEventPayload,
  ProjectIndexInvalidatedPayload,
  ResourceMutationCommittedPayload,
} from '../types';
import { parseGraphDeltaDto } from '@/shared/types/dto/editorMutationWireParser';
import { parseResourceMutationResultDto } from '@/shared/types/dto/resourceMutationResultWireParser';

type UnknownRecord = Record<string, unknown>;

export type ProjectMutationEvent = {
  type: 'Project';
  payload:
    | { type: 'GraphDelta'; payload: GraphDeltaEventPayload }
    | { type: 'ResourceMutationCommitted'; payload: ResourceMutationCommittedPayload };
};

type ProjectMutationEventType = ProjectMutationEvent['payload']['type'];

const PROJECT_MUTATION_EVENT_TYPES = {
  GraphDelta: true,
  ResourceMutationCommitted: true,
} as const satisfies Record<ProjectMutationEventType, true>;

function isRecord(value: unknown): value is UnknownRecord {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function hasExactKeys(value: UnknownRecord, keys: readonly string[]): boolean {
  return Object.keys(value).length === keys.length
    && keys.every((key) => Object.prototype.hasOwnProperty.call(value, key));
}

function assertNever(value: never): never {
  throw new Error(`Unhandled project mutation event type '${String(value)}'`);
}

function parseProjectMutationEventType(value: unknown): ProjectMutationEventType {
  if (typeof value !== 'string'
    || !Object.prototype.hasOwnProperty.call(PROJECT_MUTATION_EVENT_TYPES, value)) {
    throw new Error('Project mutation event type is malformed');
  }
  return value as ProjectMutationEventType;
}


export function parseGraphDeltaEventPayload(value: unknown): GraphDeltaEventPayload {
  if (!isRecord(value) || !hasExactKeys(value, ['projectInstanceId', 'delta'])) {
    throw new Error('GraphDelta payload must have exact projectInstanceId and delta fields');
  }
  if (typeof value.projectInstanceId !== 'string' || value.projectInstanceId.length === 0) {
    throw new Error('GraphDelta projectInstanceId is malformed');
  }
  return {
    projectInstanceId: value.projectInstanceId,
    delta: parseGraphDeltaDto(value.delta),
  };
}

export function parseProjectIndexInvalidatedPayload(
  value: unknown,
): ProjectIndexInvalidatedPayload {
  if (!isRecord(value)
    || !hasExactKeys(value, ['projectInstanceId', 'source', 'version'])) {
    throw new Error(
      'ProjectIndexInvalidated payload must have exact projectInstanceId, source, and version fields',
    );
  }
  if (typeof value.projectInstanceId !== 'string' || value.projectInstanceId.length === 0) {
    throw new Error('ProjectIndexInvalidated projectInstanceId is malformed');
  }
  if (value.source !== 'watcher') {
    throw new Error('ProjectIndexInvalidated source is malformed');
  }
  if (!Number.isSafeInteger(value.version) || (value.version as number) < 1) {
    throw new Error('ProjectIndexInvalidated watcher version is malformed');
  }
  return {
    projectInstanceId: value.projectInstanceId,
    source: value.source,
    version: value.version as number,
  };
}

export function parseResourceMutationCommittedPayload(
  value: unknown,
): ResourceMutationCommittedPayload {
  if (!isRecord(value) || !hasExactKeys(value, ['result'])) {
    throw new Error('ResourceMutationCommitted payload must have exact result field');
  }
  return { result: parseResourceMutationResultDto(value.result) };
}

export function parseProjectMutationEvent(value: unknown): ProjectMutationEvent {
  if (!isRecord(value)
    || !hasExactKeys(value, ['type', 'payload'])
    || value.type !== 'Project'
    || !isRecord(value.payload)
    || !hasExactKeys(value.payload, ['type', 'payload'])) {
    throw new Error('Project mutation event envelope is malformed');
  }

  const type = parseProjectMutationEventType(value.payload.type);
  switch (type) {
    case 'GraphDelta':
      return {
        type: 'Project',
        payload: { type, payload: parseGraphDeltaEventPayload(value.payload.payload) },
      };
    case 'ResourceMutationCommitted':
      return {
        type: 'Project',
        payload: {
          type,
          payload: parseResourceMutationCommittedPayload(value.payload.payload),
        },
      };
    default:
      return assertNever(type);
  }
}

