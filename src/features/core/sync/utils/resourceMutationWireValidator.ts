import { inferGraphResourceKind } from '@/shared/types/domain/graphResourcePath';
import type { ResourceDeltaDto } from '@/shared/types/dto/editorMutation';
import { isRustDataValueWire } from '@/shared/types/dto/dataValue';

type UnknownRecord = Record<string, unknown>;

const UUID_PATTERN = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

function isUuid(value: unknown): value is string {
  return typeof value === 'string' && UUID_PATTERN.test(value);
}

function isRecord(value: unknown): value is UnknownRecord {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function hasOwn(value: UnknownRecord, key: string): boolean {
  return Object.prototype.hasOwnProperty.call(value, key);
}

function hasExactKeys(value: UnknownRecord, keys: readonly string[]): boolean {
  return Object.keys(value).length === keys.length && keys.every((key) => hasOwn(value, key));
}

function isNullableString(value: unknown): value is string | null {
  return value === null || typeof value === 'string';
}

function isPosition(value: unknown): boolean {
  return isRecord(value)
    && typeof value.x === 'number'
    && Number.isFinite(value.x)
    && typeof value.y === 'number'
    && Number.isFinite(value.y);
}

function isDocumentNode(value: unknown): boolean {
  return isRecord(value)
    && isUuid(value.id)
    && typeof value.node_type === 'string'
    && isPosition(value.position)
    && isRecord(value.parameters)
    && isNullableString(value.user_label);
}

function isDocumentPortAddress(value: unknown): boolean {
  if (!isRecord(value)
    || !isUuid(value.node_id)
    || !isRecord(value.port)) return false;
  if (value.port.kind === 'declared') return typeof value.port.key === 'string';
  return value.port.kind === 'instance'
    && typeof value.port.template === 'string'
    && isUuid(value.port.instance_id);
}

function isDynamicMemberLocator(value: unknown): boolean {
  if (!isRecord(value)) return false;
  if (value.kind === 'function_parameter') {
    return typeof value.function === 'string' && typeof value.parameter === 'string';
  }
  return value.kind === 'schema_field'
    && typeof value.source === 'string'
    && typeof value.field === 'string';
}

function isDynamicPortBinding(value: unknown): boolean {
  if (!isRecord(value) || typeof value.order !== 'string') return false;
  if (value.kind === 'user_created') return true;
  if (value.kind === 'resolved') return isDynamicMemberLocator(value.origin);
  return value.kind === 'orphan'
    && isDynamicMemberLocator(value.origin)
    && isRecord(value.last_known)
    && typeof value.last_known.label === 'string';
}

function isDocumentConnection(value: unknown): boolean {
  return isRecord(value)
    && isUuid(value.id)
    && isDocumentPortAddress(value.output)
    && isDocumentPortAddress(value.input)
    && isNullableString(value.order);
}

function isInputState(value: unknown): boolean {
  return isRecord(value) && hasOwn(value, 'literal_override');
}

function isNullableInputState(value: unknown): boolean {
  return value === null || isInputState(value);
}

function isGraphOperation(value: unknown): boolean {
  if (!isRecord(value) || typeof value.operation !== 'string') return false;
  switch (value.operation) {
    case 'insert_node':
    case 'remove_node':
      return isDocumentNode(value.node);
    case 'update_node':
      return isDocumentNode(value.before) && isDocumentNode(value.after);
    case 'insert_port_binding':
    case 'remove_port_binding':
      return isDocumentPortAddress(value.address) && isDynamicPortBinding(value.binding);
    case 'insert_connection':
    case 'remove_connection':
      return isDocumentConnection(value.connection);
    case 'set_input_state':
      return isDocumentPortAddress(value.address)
        && isNullableInputState(value.before)
        && isNullableInputState(value.after);
    default:
      return false;
  }
}

function isGraphPatch(value: unknown): boolean {
  return isRecord(value)
    && Array.isArray(value.operations)
    && value.operations.every(isGraphOperation);
}

function isFunctionParameter(value: unknown): boolean {
  return isRecord(value)
    && typeof value.id === 'string'
    && typeof value.name === 'string'
    && typeof value.type_name === 'string';
}

function isFunctionSignature(value: unknown): boolean {
  return isRecord(value)
    && Array.isArray(value.parameters)
    && value.parameters.every(isFunctionParameter)
    && isNullableString(value.return_type);
}

function isFunctionPatch(value: unknown): boolean {
  return isRecord(value)
    && isFunctionSignature(value.before)
    && isFunctionSignature(value.after);
}

function isVariableDocument(value: unknown): boolean {
  if (!isRecord(value)
    || !isUuid(value.id)
    || typeof value.name !== 'string'
    || !hasOwn(value, 'dataType')
    || !isRustDataValueWire(value.dataValue)
    || typeof value.description !== 'string'
    || !Array.isArray(value.tags)
    || !value.tags.every((tag) => typeof tag === 'string')
    || !isRecord(value.scope)) return false;
  return value.scope.type === 'global'
    || (value.scope.type === 'event' && typeof value.scope.eventPath === 'string')
    || (value.scope.type === 'function' && typeof value.scope.functionPath === 'string');
}

function isVariablePatch(value: unknown): boolean {
  return isRecord(value)
    && Object.keys(value).length === 2
    && hasOwn(value, 'before')
    && hasOwn(value, 'after')
    && (value.before === null || isVariableDocument(value.before))
    && (value.after === null || isVariableDocument(value.after))
    && !(value.before === null && value.after === null);
}

function isSqlEngine(value: unknown): boolean {
  if (!isRecord(value) || Object.keys(value).length !== 1) return false;
  if (isRecord(value.sqlite)) {
    return hasExactKeys(value.sqlite, ['autoCreate']) && typeof value.sqlite.autoCreate === 'boolean';
  }
  if (isRecord(value.postgres)) {
    return hasExactKeys(value.postgres, ['ssl']) && typeof value.postgres.ssl === 'boolean';
  }
  return isRecord(value.mysql)
    && hasExactKeys(value.mysql, ['charset'])
    && typeof value.mysql.charset === 'string';
}

function isDatabaseEngine(value: unknown): boolean {
  if (!isRecord(value) || Object.keys(value).length !== 1) return false;
  if (isRecord(value.csv)) {
    return hasExactKeys(value.csv, ['path', 'delimiter', 'hasHeader', 'inferSchemaLength'])
      && typeof value.csv.path === 'string'
      && typeof value.csv.delimiter === 'string'
      && [...value.csv.delimiter].length === 1
      && typeof value.csv.hasHeader === 'boolean'
      && (value.csv.inferSchemaLength === null
        || (Number.isSafeInteger(value.csv.inferSchemaLength)
          && (value.csv.inferSchemaLength as number) >= 0));
  }
  if (isRecord(value.sql)) {
    return hasExactKeys(value.sql, ['engine', 'connectionString', 'table'])
      && isSqlEngine(value.sql.engine)
      && typeof value.sql.connectionString === 'string'
      && typeof value.sql.table === 'string';
  }
  if (isRecord(value.parquet)) {
    return hasExactKeys(value.parquet, ['path', 'columns'])
      && typeof value.parquet.path === 'string'
      && (value.parquet.columns === null
        || (Array.isArray(value.parquet.columns)
          && value.parquet.columns.every((column) => typeof column === 'string')));
  }
  if (isRecord(value.excel)) {
    return hasExactKeys(value.excel, ['path', 'sheet'])
      && typeof value.excel.path === 'string'
      && typeof value.excel.sheet === 'string';
  }
  if (isRecord(value.duckDb)) {
    return hasExactKeys(value.duckDb, ['path', 'table'])
      && typeof value.duckDb.path === 'string'
      && typeof value.duckDb.table === 'string';
  }
  return isRecord(value.inMemory)
    && hasExactKeys(value.inMemory, ['name'])
    && typeof value.inMemory.name === 'string';
}

function isDatabaseDocument(value: unknown): value is UnknownRecord {
  return isRecord(value)
    && hasExactKeys(value, ['id', 'engine', 'schemaVersion', 'required', 'name'])
    && typeof value.id === 'string'
    && value.id.length > 0
    && isDatabaseEngine(value.engine)
    && Number.isSafeInteger(value.schemaVersion)
    && (value.schemaVersion as number) >= 0
    && typeof value.required === 'boolean'
    && isNullableString(value.name);
}

function isDatabasePatch(value: unknown): boolean {
  if (!isRecord(value)
    || !hasExactKeys(value, ['before', 'after'])
    || (value.before !== null && !isDatabaseDocument(value.before))
    || (value.after !== null && !isDatabaseDocument(value.after))
    || (value.before === null && value.after === null)) return false;
  return value.before === null
    || value.after === null
    || value.before.id === value.after.id;
}

function isVariableResourceKey(value: unknown): value is string {
  if (typeof value !== 'string') return false;
  const [prefix, id, ...rest] = value.split('/');
  return prefix === 'variables' && rest.length === 0 && isUuid(id);
}

function isGraphPath(value: unknown): value is string {
  return typeof value === 'string' && inferGraphResourceKind(value) != null;
}

function isResourcePathMovePatch(value: unknown): boolean {
  return isRecord(value)
    && isGraphPath(value.from)
    && isGraphPath(value.to)
    && value.from !== value.to;
}

function isGraphResourceLifecycleState(value: unknown, path: string): boolean {
  if (!isRecord(value)
    || !Number.isSafeInteger(value.revision)
    || (value.revision as number) < 0
    || value.path !== path) return false;
  return value.kind === inferGraphResourceKind(path);
}

function isGraphResourceLifecyclePatch(value: unknown, path: string): boolean {
  if (!isRecord(value)
    || Object.keys(value).length !== 2
    || !hasOwn(value, 'before')
    || !hasOwn(value, 'after')) return false;
  const beforeValid = value.before === null || isGraphResourceLifecycleState(value.before, path);
  const afterValid = value.after === null || isGraphResourceLifecycleState(value.after, path);
  return beforeValid && afterValid && (value.before === null) !== (value.after === null);
}

function isOperationCorrelation(value: unknown): value is string | null {
  return value === null || isUuid(value);
}

function isResourceAndPayload(value: UnknownRecord): boolean {
  if (!isRecord(value.resource) || !isRecord(value.payload)) return false;
  const { kind, key } = value.resource;
  if (kind === 'graph') {
    return isGraphPath(key)
      && ((value.payload.kind === 'graph' && isGraphPatch(value.payload.patch))
        || (value.payload.kind === 'graph_resource_lifecycle'
          && isGraphResourceLifecyclePatch(value.payload.patch, key))
        || (value.payload.kind === 'graph_resource_move'
          && isResourcePathMovePatch(value.payload.patch)));
  }
  if (kind === 'function') {
    return isGraphPath(key)
      && value.payload.kind === 'function'
      && isFunctionPatch(value.payload.patch);
  }
  if (kind === 'variable') {
    return isVariableResourceKey(key)
      && ((value.payload.kind === 'variable' && isVariablePatch(value.payload.patch))
        || (value.payload.kind === 'variable_scope_move'
          && isResourcePathMovePatch(value.payload.patch)));
  }
  return kind === 'database'
    && hasExactKeys(value.resource, ['kind', 'key'])
    && typeof key === 'string'
    && key.length > 0
    && hasExactKeys(value.payload, ['kind', 'patch'])
    && value.payload.kind === 'database'
    && isDatabasePatch(value.payload.patch);
}

function isResourceDelta(value: unknown): value is ResourceDeltaDto {
  if (!isRecord(value)
    || !hasExactKeys(value, ['resource', 'fromRevision', 'toRevision', 'causedBy', 'payload'])
    || !isResourceAndPayload(value)) return false;
  if (!Number.isSafeInteger(value.fromRevision)
    || !Number.isSafeInteger(value.toRevision)
    || (value.fromRevision as number) < 0
    || value.toRevision !== (value.fromRevision as number) + 1
    || !isOperationCorrelation(value.causedBy)) return false;
  if (isRecord(value.payload) && value.payload.kind === 'graph_resource_lifecycle') {
    const patch = value.payload.patch as UnknownRecord;
    const present = patch.before ?? patch.after;
    return isUuid(value.causedBy)
      && isRecord(present)
      && present.revision === value.fromRevision;
  }
  return true;
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
