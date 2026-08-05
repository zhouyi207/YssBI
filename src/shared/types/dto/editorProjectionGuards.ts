import { isSchemaAwareParameterEditorDto } from './parameterEditorValidators';
import type {
  DiagnosticLocationDto,
  EditorGraphProjectionDto,
  PortAddressDto,
} from './editorProjection';

const fingerprintPattern = /^[0-9a-f]{64}$/;
const portDirections = new Set(['input', 'output']);
const portKinds = new Set(['data', 'control', 'effect']);
const portInstanceKinds = new Set(['declared', 'userCreated', 'derived']);
const bindingKinds = new Set(['connections', 'literal', 'protocolDefault', 'unbound']);
const scalarTypes = new Set([
  'boolean', 'int64', 'float64', 'string', 'date', 'dateTime', 'unknown',
]);
const schemaKinds = new Set(['input', 'project', 'append', 'rename', 'filter', 'derived']);
const portStatuses = new Set(['resolved', 'orphan']);
const parameterEditorKinds = new Set(['auto', 'text', 'number', 'toggle', 'select', 'resource']);
const diagnosticSeverities = new Set(['error', 'warning', 'information']);

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function hasExactKeys(value: unknown, keys: readonly string[]): value is Record<string, unknown> {
  if (!isRecord(value)) return false;
  const actual = Object.keys(value);
  return actual.length === keys.length && keys.every((key) => (
    Object.prototype.hasOwnProperty.call(value, key)
  ));
}

function isStringOrNull(value: unknown): value is string | null {
  return value === null || typeof value === 'string';
}

function isNonNegativeSafeInteger(value: unknown): value is number {
  return Number.isSafeInteger(value) && (value as number) >= 0;
}

function isJsonValue(value: unknown): boolean {
  if (value === null || typeof value === 'string' || typeof value === 'boolean') return true;
  if (typeof value === 'number') return Number.isFinite(value);
  if (Array.isArray(value)) return value.every(isJsonValue);
  return isRecord(value) && Object.values(value).every(isJsonValue);
}

function isStringRecord(value: unknown): value is Record<string, string> {
  return isRecord(value) && Object.values(value).every((entry) => typeof entry === 'string');
}

function isProjectionBasis(value: unknown): boolean {
  return hasExactKeys(value, [
    'graphPath', 'graphRevision', 'registryFingerprint', 'resourceVersions',
  ])
    && typeof value.graphPath === 'string'
    && isNonNegativeSafeInteger(value.graphRevision)
    && typeof value.registryFingerprint === 'string'
    && fingerprintPattern.test(value.registryFingerprint)
    && isStringRecord(value.resourceVersions);
}

function isPosition(value: unknown): boolean {
  return hasExactKeys(value, ['x', 'y'])
    && typeof value.x === 'number' && Number.isFinite(value.x)
    && typeof value.y === 'number' && Number.isFinite(value.y);
}

function isNodeDisplay(value: unknown): boolean {
  return hasExactKeys(value, ['title', 'description', 'userLabel', 'iconId', 'styleId'])
    && typeof value.title === 'string'
    && isStringOrNull(value.description)
    && isStringOrNull(value.userLabel)
    && isStringOrNull(value.iconId)
    && isStringOrNull(value.styleId);
}

function isCapabilities(value: unknown): boolean {
  if (!hasExactKeys(value, [
    'managed', 'canCopy', 'canDelete', 'canEditLabel', 'canEditParameters',
    'hasDynamicPorts', 'supportsInlineLiterals',
  ])) return false;
  return Object.values(value).every((entry) => typeof entry === 'boolean');
}

function isPortAddress(value: unknown): value is PortAddressDto {
  if (!isRecord(value)) return false;
  if (value.kind === 'declared') {
    return hasExactKeys(value, ['kind', 'nodeId', 'portKey'])
      && typeof value.nodeId === 'string'
      && typeof value.portKey === 'string';
  }
  if (value.kind === 'instance') {
    return hasExactKeys(value, ['kind', 'nodeId', 'templateKey', 'instanceId'])
      && typeof value.nodeId === 'string'
      && typeof value.templateKey === 'string'
      && typeof value.instanceId === 'string';
  }
  return false;
}

function isPortDisplay(value: unknown): boolean {
  return hasExactKeys(value, ['label', 'instanceLabel'])
    && typeof value.label === 'string'
    && isStringOrNull(value.instanceLabel);
}

function isConnectionCapability(value: unknown): boolean {
  return hasExactKeys(value, ['current', 'maximum', 'ordered', 'canConnect'])
    && isNonNegativeSafeInteger(value.current)
    && (value.maximum === null || isNonNegativeSafeInteger(value.maximum))
    && typeof value.ordered === 'boolean'
    && typeof value.canConnect === 'boolean';
}

function isInputBinding(value: unknown): boolean {
  return hasExactKeys(value, ['literalOverride', 'protocolDefault', 'effective'])
    && isJsonValue(value.literalOverride)
    && isJsonValue(value.protocolDefault)
    && bindingKinds.has(value.effective as string);
}

function isTypeSummary(value: unknown): boolean {
  return hasExactKeys(value, ['display', 'resolved'])
    && typeof value.display === 'string'
    && typeof value.resolved === 'boolean';
}

function isSchemaSummary(value: unknown): boolean {
  return hasExactKeys(value, ['kind', 'fields'])
    && schemaKinds.has(value.kind as string)
    && Array.isArray(value.fields)
    && value.fields.every((field) => hasExactKeys(field, ['name', 'scalarType'])
      && typeof field.name === 'string'
      && scalarTypes.has(field.scalarType as string));
}

function isPort(value: unknown): boolean {
  return hasExactKeys(value, [
    'address', 'templateKey', 'display', 'direction', 'kind', 'instanceKind', 'orphan',
    'canRemove', 'connections', 'input', 'resolvedType', 'resolvedSchema', 'status',
  ])
    && isPortAddress(value.address)
    && typeof value.templateKey === 'string'
    && isPortDisplay(value.display)
    && portDirections.has(value.direction as string)
    && portKinds.has(value.kind as string)
    && portInstanceKinds.has(value.instanceKind as string)
    && typeof value.orphan === 'boolean'
    && typeof value.canRemove === 'boolean'
    && isConnectionCapability(value.connections)
    && (value.input === null || isInputBinding(value.input))
    && (value.resolvedType === null || isTypeSummary(value.resolvedType))
    && (value.resolvedSchema === null || isSchemaSummary(value.resolvedSchema))
    && portStatuses.has(value.status as string);
}

function isParameterEditor(value: unknown): boolean {
  return hasExactKeys(value, [
    'key', 'display', 'editor', 'multiline', 'value', 'configuration',
  ])
    && typeof value.key === 'string'
    && hasExactKeys(value.display, ['title', 'description'])
    && typeof value.display.title === 'string'
    && isStringOrNull(value.display.description)
    && parameterEditorKinds.has(value.editor as string)
    && typeof value.multiline === 'boolean'
    && isJsonValue(value.value)
    && (value.configuration === null || isSchemaAwareParameterEditorDto(value.configuration));
}

function isDiagnosticLocation(value: unknown): value is DiagnosticLocationDto {
  if (!isRecord(value)) return false;
  switch (value.kind) {
    case 'graph':
      return hasExactKeys(value, ['kind']);
    case 'node':
      return hasExactKeys(value, ['kind', 'nodeId']) && typeof value.nodeId === 'string';
    case 'port':
      return hasExactKeys(value, ['kind', 'address']) && isPortAddress(value.address);
    case 'connection':
      return hasExactKeys(value, ['kind', 'connectionId'])
        && typeof value.connectionId === 'string';
    case 'parameter':
      return hasExactKeys(value, ['kind', 'nodeId', 'key'])
        && typeof value.nodeId === 'string' && typeof value.key === 'string';
    case 'resource':
      return hasExactKeys(value, ['kind', 'identity']) && typeof value.identity === 'string';
    default:
      return false;
  }
}

function isDiagnostic(value: unknown): boolean {
  return hasExactKeys(value, ['code', 'message', 'severity', 'blocking', 'location', 'related'])
    && typeof value.code === 'string'
    && typeof value.message === 'string'
    && diagnosticSeverities.has(value.severity as string)
    && typeof value.blocking === 'boolean'
    && isDiagnosticLocation(value.location)
    && Array.isArray(value.related) && value.related.every(isDiagnosticLocation);
}

function isNode(value: unknown): boolean {
  return hasExactKeys(value, [
    'graphPath', 'sourceRevision', 'nodeId', 'nodeTypeId', 'position', 'display', 'ports',
    'parameterEditors', 'capabilities', 'diagnostics',
  ])
    && typeof value.graphPath === 'string'
    && isNonNegativeSafeInteger(value.sourceRevision)
    && typeof value.nodeId === 'string'
    && typeof value.nodeTypeId === 'string'
    && isPosition(value.position)
    && isNodeDisplay(value.display)
    && Array.isArray(value.ports) && value.ports.every(isPort)
    && Array.isArray(value.parameterEditors) && value.parameterEditors.every(isParameterEditor)
    && isCapabilities(value.capabilities)
    && Array.isArray(value.diagnostics) && value.diagnostics.every(isDiagnostic);
}

function isConnection(value: unknown): boolean {
  return hasExactKeys(value, ['connectionId', 'output', 'input', 'order'])
    && typeof value.connectionId === 'string'
    && isPortAddress(value.output)
    && isPortAddress(value.input)
    && isStringOrNull(value.order);
}

export function isEditorGraphProjectionDto(value: unknown): value is EditorGraphProjectionDto {
  return hasExactKeys(value, [
    'basis', 'graphPath', 'sourceRevision', 'nodes', 'connections', 'diagnostics',
    'hasBlockingDiagnostics',
  ])
    && isProjectionBasis(value.basis)
    && typeof value.graphPath === 'string'
    && isNonNegativeSafeInteger(value.sourceRevision)
    && Array.isArray(value.nodes) && value.nodes.every(isNode)
    && Array.isArray(value.connections) && value.connections.every(isConnection)
    && Array.isArray(value.diagnostics) && value.diagnostics.every(isDiagnostic)
    && typeof value.hasBlockingDiagnostics === 'boolean';
}
