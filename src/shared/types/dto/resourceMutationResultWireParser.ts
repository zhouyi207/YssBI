import { areResourceDeltasValid } from './resourceMutationWireValidator';
import { validateResourceMutationWireResult } from './resourceMutationResultValidator';

import type {
  ProjectionStatusDto,
  ResourceDeltaDto,
  ResourceDocumentPatchDto,
  ResourceKeyDto,
  ResourceMoveDto,
  ResourceMutationResultDto,
} from '@/shared/types/dto/editorMutation';
import {
  isTypeExprWire,
  parseGraphProjectionReplacementDto,
  parseHistoryStatusDto,
} from '@/shared/types/dto/editorMutationWireParser';

type UnknownRecord = Record<string, unknown>;

function assertNever(value: never): never {
  throw new Error(`Unhandled resource mutation variant '${String(value)}'`);
}

function isRecord(value: unknown): value is UnknownRecord {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function hasExactKeys(value: UnknownRecord, keys: readonly string[]): boolean {
  return Object.keys(value).length === keys.length
    && keys.every((key) => Object.prototype.hasOwnProperty.call(value, key));
}


function isSafeInteger(value: unknown): value is number {
  return Number.isSafeInteger(value);
}

function isJsonValue(value: unknown): boolean {
  if (value === null || typeof value === 'string' || typeof value === 'boolean') return true;
  if (typeof value === 'number') return Number.isFinite(value);
  if (Array.isArray(value)) return value.every(isJsonValue);
  return isRecord(value) && Object.values(value).every(isJsonValue);
}

function isPositionShape(value: unknown): boolean {
  return isRecord(value) && hasExactKeys(value, ['x', 'y']);
}

function isDocumentNodeShape(value: unknown): boolean {
  return isRecord(value)
    && hasExactKeys(value, ['id', 'node_type', 'position', 'parameters', 'user_label'])
    && isPositionShape(value.position)
    && isRecord(value.parameters);
}

function isDocumentPortAddressShape(value: unknown): boolean {
  if (!isRecord(value) || !hasExactKeys(value, ['node_id', 'port']) || !isRecord(value.port)) {
    return false;
  }
  if (value.port.kind === 'declared') return hasExactKeys(value.port, ['kind', 'key']);
  return value.port.kind === 'instance'
    && hasExactKeys(value.port, ['kind', 'template', 'instance_id']);
}

function isDynamicMemberLocatorShape(value: unknown): boolean {
  if (!isRecord(value)) return false;
  if (value.kind === 'function_parameter') {
    return hasExactKeys(value, ['kind', 'function', 'parameter']);
  }
  return value.kind === 'schema_field'
    && hasExactKeys(value, ['kind', 'source', 'field']);
}

function isLastKnownPortMetadataShape(value: unknown): boolean {
  if (!isRecord(value) || typeof value.label !== 'string') return false;
  return hasExactKeys(value, ['label'])
    || (hasExactKeys(value, ['label', 'value_type']) && isTypeExprWire(value.value_type));
}

function isDynamicPortBindingShape(value: unknown): boolean {
  if (!isRecord(value)) return false;
  if (value.kind === 'user_created') return hasExactKeys(value, ['kind', 'order']);
  if (value.kind === 'resolved') {
    return isDynamicMemberLocatorShape(value.origin)
      && (hasExactKeys(value, ['kind', 'origin', 'order'])
        || (hasExactKeys(value, ['kind', 'origin', 'order', 'last_known'])
          && isLastKnownPortMetadataShape(value.last_known)));
  }
  return value.kind === 'orphan'
    && hasExactKeys(value, ['kind', 'origin', 'order', 'last_known'])
    && isDynamicMemberLocatorShape(value.origin)
    && isLastKnownPortMetadataShape(value.last_known);
}

function isDocumentConnectionShape(value: unknown): boolean {
  return isRecord(value)
    && hasExactKeys(value, ['id', 'output', 'input', 'order'])
    && isDocumentPortAddressShape(value.output)
    && isDocumentPortAddressShape(value.input);
}

function isInputStateShape(value: unknown): boolean {
  return isRecord(value)
    && hasExactKeys(value, ['literal_override'])
    && isJsonValue(value.literal_override);
}

function isGraphOperationShape(value: unknown): boolean {
  if (!isRecord(value) || typeof value.operation !== 'string') return false;
  switch (value.operation) {
    case 'insert_node':
    case 'remove_node':
      return hasExactKeys(value, ['operation', 'node']) && isDocumentNodeShape(value.node);
    case 'update_node':
      return hasExactKeys(value, ['operation', 'before', 'after'])
        && isDocumentNodeShape(value.before)
        && isDocumentNodeShape(value.after);
    case 'insert_port_binding':
    case 'remove_port_binding':
      return hasExactKeys(value, ['operation', 'address', 'binding'])
        && isDocumentPortAddressShape(value.address)
        && isDynamicPortBindingShape(value.binding);
    case 'insert_connection':
    case 'remove_connection':
      return hasExactKeys(value, ['operation', 'connection'])
        && isDocumentConnectionShape(value.connection);
    case 'set_input_state':
      return hasExactKeys(value, ['operation', 'address', 'before', 'after'])
        && isDocumentPortAddressShape(value.address)
        && (value.before === null || isInputStateShape(value.before))
        && (value.after === null || isInputStateShape(value.after));
    default:
      return false;
  }
}

function isGraphPatchShape(value: unknown): boolean {
  return isRecord(value)
    && hasExactKeys(value, ['operations'])
    && Array.isArray(value.operations)
    && value.operations.every(isGraphOperationShape);
}

function isFunctionSignatureShape(value: unknown): boolean {
  return isRecord(value)
    && hasExactKeys(value, ['parameters', 'return_type'])
    && Array.isArray(value.parameters)
    && value.parameters.every((parameter) => isRecord(parameter)
      && hasExactKeys(parameter, ['id', 'name', 'type_name']));
}

function isFunctionPatchShape(value: unknown): boolean {
  return isRecord(value)
    && hasExactKeys(value, ['before', 'after'])
    && isFunctionSignatureShape(value.before)
    && isFunctionSignatureShape(value.after);
}

function isBeforeAfterShape(value: unknown): boolean {
  return isRecord(value)
    && hasExactKeys(value, ['before', 'after'])
    && isJsonValue(value.before)
    && isJsonValue(value.after);
}

function isPathMoveShape(value: unknown): boolean {
  return isRecord(value) && hasExactKeys(value, ['from', 'to']);
}

function isLifecycleStateShape(value: unknown): boolean {
  return isRecord(value) && hasExactKeys(value, ['revision', 'path', 'kind', 'name']);
}

function isLifecyclePatchShape(value: unknown): boolean {
  return isRecord(value)
    && hasExactKeys(value, ['before', 'after'])
    && (value.before === null || isLifecycleStateShape(value.before))
    && (value.after === null || isLifecycleStateShape(value.after));
}

function isSqlEngineShape(value: unknown): boolean {
  if (!isRecord(value) || Object.keys(value).length !== 1) return false;
  if (isRecord(value.sqlite)) return hasExactKeys(value.sqlite, ['autoCreate']);
  if (isRecord(value.postgres)) return hasExactKeys(value.postgres, ['ssl']);
  return isRecord(value.mysql) && hasExactKeys(value.mysql, ['charset']);
}

function isDatabaseEngineShape(value: unknown): boolean {
  if (!isRecord(value) || Object.keys(value).length !== 1) return false;
  if (isRecord(value.csv)) {
    return hasExactKeys(value.csv, ['path', 'delimiter', 'hasHeader', 'inferSchemaLength']);
  }
  if (isRecord(value.sql)) {
    return hasExactKeys(value.sql, ['engine', 'connectionString', 'table'])
      && isSqlEngineShape(value.sql.engine);
  }
  if (isRecord(value.parquet)) return hasExactKeys(value.parquet, ['path', 'columns']);
  if (isRecord(value.excel)) return hasExactKeys(value.excel, ['path', 'sheet']);
  if (isRecord(value.duckDb)) return hasExactKeys(value.duckDb, ['path', 'table']);
  return isRecord(value.inMemory) && hasExactKeys(value.inMemory, ['name']);
}

function isDatabaseDocumentShape(value: unknown): boolean {
  return isRecord(value)
    && hasExactKeys(value, ['id', 'engine', 'schemaVersion', 'required', 'name'])
    && isDatabaseEngineShape(value.engine);
}

function isDatabasePatchShape(value: unknown): boolean {
  return isRecord(value)
    && hasExactKeys(value, ['before', 'after'])
    && (value.before === null || isDatabaseDocumentShape(value.before))
    && (value.after === null || isDatabaseDocumentShape(value.after));
}

function isWorksheetDocumentStateShape(value: unknown): boolean {
  return isRecord(value)
    && hasExactKeys(value, ['databaseId', 'chartType', 'encodings'])
    && isRecord(value.encodings)
    && Object.keys(value.encodings).every((key) => key === 'x' || key === 'y');
}

function isWorksheetPatchShape(value: unknown): boolean {
  return isRecord(value)
    && hasExactKeys(value, ['before', 'after'])
    && isWorksheetDocumentStateShape(value.before)
    && isWorksheetDocumentStateShape(value.after);
}

function isResourcePayloadShape(value: unknown): boolean {
  if (!isRecord(value) || !hasExactKeys(value, ['kind', 'patch'])) return false;
  switch (value.kind) {
    case 'graph': return isGraphPatchShape(value.patch);
    case 'function': return isFunctionPatchShape(value.patch);
    case 'worksheet': return isWorksheetPatchShape(value.patch);
    case 'resource_lifecycle': return isLifecyclePatchShape(value.patch);
    case 'resource_move':
    case 'variable_scope_move': return isPathMoveShape(value.patch);
    case 'variable': return isBeforeAfterShape(value.patch);
    case 'database': return isDatabasePatchShape(value.patch);
    default: return false;
  }
}

function hasExactResourceDeltaShape(value: unknown): boolean {
  return isRecord(value)
    && hasExactKeys(value, ['resource', 'fromRevision', 'toRevision', 'causedBy', 'payload'])
    && isRecord(value.resource)
    && hasExactKeys(value.resource, ['kind', 'key'])
    && isResourcePayloadShape(value.payload);
}

function cloneResourceKey(resource: ResourceKeyDto): ResourceKeyDto {
  switch (resource.kind) {
    case 'graph': return { kind: 'graph', key: resource.key };
    case 'function': return { kind: 'function', key: resource.key };
    case 'variable': return { kind: 'variable', key: resource.key };
    case 'database': return { kind: 'database', key: resource.key };
    case 'worksheet': return { kind: 'worksheet', key: resource.key };
    default: return assertNever(resource);
  }
}

function cloneResourcePayload(payload: ResourceDocumentPatchDto): ResourceDocumentPatchDto {
  switch (payload.kind) {
    case 'graph': return { kind: 'graph', patch: structuredClone(payload.patch) };
    case 'function': return { kind: 'function', patch: structuredClone(payload.patch) };
    case 'worksheet': return { kind: 'worksheet', patch: structuredClone(payload.patch) };
    case 'resource_lifecycle':
      return { kind: 'resource_lifecycle', patch: structuredClone(payload.patch) };
    case 'resource_move':
      return { kind: 'resource_move', patch: structuredClone(payload.patch) };
    case 'variable': return { kind: 'variable', patch: structuredClone(payload.patch) };
    case 'variable_scope_move':
      return { kind: 'variable_scope_move', patch: structuredClone(payload.patch) };
    case 'database': return { kind: 'database', patch: structuredClone(payload.patch) };
    default: return assertNever(payload);
  }
}

function parseResourceDeltas(value: unknown): ResourceDeltaDto[] {
  if (!areResourceDeltasValid(value) || !value.every(hasExactResourceDeltaShape)) {
    throw new Error('resource deltas are malformed');
  }
  return value.map((delta) => ({
    resource: cloneResourceKey(delta.resource),
    fromRevision: delta.fromRevision,
    toRevision: delta.toRevision,
    causedBy: delta.causedBy,
    payload: cloneResourcePayload(delta.payload),
  }));
}

function parseMoves(value: unknown): ResourceMoveDto[] {
  if (!Array.isArray(value)) throw new Error('resource moves are malformed');
  return value.map((move) => {
    if (!isRecord(move)
      || !hasExactKeys(move, ['from', 'to', 'kind', 'name'])
      || typeof move.from !== 'string'
      || typeof move.to !== 'string'
      || (move.kind !== 'event' && move.kind !== 'function' && move.kind !== 'worksheet')
      || typeof move.name !== 'string') throw new Error('resource moves are malformed');
    return { from: move.from, to: move.to, kind: move.kind, name: move.name };
  });
}

function parseProjectionStatus(value: unknown): ProjectionStatusDto {
  if (!isRecord(value) || typeof value.status !== 'string') {
    throw new Error('projection status is malformed');
  }
  switch (value.status) {
    case 'complete':
      if (!hasExactKeys(value, ['status', 'expectedGraphPaths'])
        || !Array.isArray(value.expectedGraphPaths)
        || !value.expectedGraphPaths.every((path) => typeof path === 'string')) {
        throw new Error('projection status is malformed');
      }
      return { status: 'complete', expectedGraphPaths: [...value.expectedGraphPaths] };
    case 'incomplete':
      if (!hasExactKeys(value, ['status', 'invalidatedGraphPaths'])
        || !Array.isArray(value.invalidatedGraphPaths)
        || !value.invalidatedGraphPaths.every((path) => typeof path === 'string')) {
        throw new Error('projection status is malformed');
      }
      return { status: 'incomplete', invalidatedGraphPaths: [...value.invalidatedGraphPaths] };
    default:
      throw new Error('projection status is malformed');
  }
}


export function parseResourceMutationResultDto(value: unknown): ResourceMutationResultDto {
  if (!isRecord(value)) throw new Error('resource mutation result is malformed');
  const keys = [
    'operationId', 'projectInstanceId', 'publicationRevision', 'moves', 'deltas',
    'projectionReplacements', 'projectionStatus', 'history',
  ];
  if (!hasExactKeys(value, keys)
    || typeof value.operationId !== 'string'
    || typeof value.projectInstanceId !== 'string'
    || !isSafeInteger(value.publicationRevision)) {
    throw new Error('resource mutation result is malformed');
  }

  const result: ResourceMutationResultDto = {
    operationId: value.operationId,
    projectInstanceId: value.projectInstanceId,
    publicationRevision: value.publicationRevision,
    moves: parseMoves(value.moves),
    deltas: parseResourceDeltas(value.deltas),
    projectionReplacements: Array.isArray(value.projectionReplacements)
      ? value.projectionReplacements.map((replacement) => (
        structuredClone(parseGraphProjectionReplacementDto(replacement))
      ))
      : (() => { throw new Error('projection replacements are malformed'); })(),
    projectionStatus: parseProjectionStatus(value.projectionStatus),
    history: parseHistoryStatusDto(value.history),
  };

  const validationError = validateResourceMutationWireResult(result);
  if (validationError) throw new Error(validationError);
  return result;
}
