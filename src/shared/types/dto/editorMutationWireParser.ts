import type {
  GraphDeltaDto,
  EditorGraphMutationDto,
  GraphDocumentPatchDto,
  GraphMutationResultDto,
  HistoryStatusDto,
  TypeExprDto,
} from './editorMutation';
import type {
  DiagnosticLocationDto,
  EditorGraphProjectionDto,
  GraphProjectionReplacementDto,
} from './editorProjection';
import { isDataTypeBackendFormat } from './dataType';
import {
  isFunctionEditorProjectionDto,
  isGraphResourcePath,
  isPortAddressDto,
  isUuid,
} from './editorProjectionGuards';

type UnknownRecord = Record<string, unknown>;

const FINGERPRINT_PATTERN = /^[0-9a-f]{64}$/;

function isRecord(value: unknown): value is UnknownRecord {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function hasExactKeys(value: UnknownRecord, keys: readonly string[]): boolean {
  return Object.keys(value).length === keys.length
    && keys.every((key) => Object.prototype.hasOwnProperty.call(value, key));
}

function isSafeRevision(value: unknown): value is number {
  return Number.isSafeInteger(value) && (value as number) >= 0;
}

function isNullableString(value: unknown): value is string | null {
  return value === null || typeof value === 'string';
}

function isFiniteNumber(value: unknown): value is number {
  return typeof value === 'number' && Number.isFinite(value);
}

function isJsonValue(value: unknown): boolean {
  if (value === null || typeof value === 'string' || typeof value === 'boolean') return true;
  if (isFiniteNumber(value)) return true;
  if (Array.isArray(value)) return value.every(isJsonValue);
  return isRecord(value) && Object.values(value).every(isJsonValue);
}

function isPosition(value: unknown): boolean {
  return isRecord(value)
    && hasExactKeys(value, ['x', 'y'])
    && isFiniteNumber(value.x)
    && isFiniteNumber(value.y);
}

export function parseEditorGraphMutationDto(
  value: unknown,
): Extract<EditorGraphMutationDto, { type: 'insertReroute' }> {
  if (!isRecord(value)
    || value.type !== 'insertReroute'
    || !hasExactKeys(value, ['type', 'payload'])
    || !isRecord(value.payload)
    || !hasExactKeys(value.payload, ['connectionId', 'position'])
    || typeof value.payload.connectionId !== 'string'
    || value.payload.connectionId.trim().length === 0
    || !isPosition(value.payload.position)) {
    throw new Error('InsertReroute mutation must have exact connectionId and finite position fields');
  }

  return {
    type: 'insertReroute',
    payload: {
      connectionId: value.payload.connectionId,
      position: {
        x: (value.payload.position as { x: number }).x,
        y: (value.payload.position as { y: number }).y,
      },
    },
  };
}

function isDocumentPortAddress(value: unknown): boolean {
  if (!isRecord(value)
    || !hasExactKeys(value, ['node_id', 'port'])
    || !isUuid(value.node_id)
    || !isRecord(value.port)) return false;
  if (value.port.kind === 'declared') {
    return hasExactKeys(value.port, ['kind', 'key']) && typeof value.port.key === 'string';
  }
  return value.port.kind === 'instance'
    && hasExactKeys(value.port, ['kind', 'template', 'instance_id'])
    && typeof value.port.template === 'string'
    && isUuid(value.port.instance_id);
}

function isDocumentNode(value: unknown): boolean {
  return isRecord(value)
    && hasExactKeys(value, ['id', 'node_type', 'position', 'parameters', 'user_label'])
    && isUuid(value.id)
    && typeof value.node_type === 'string'
    && isPosition(value.position)
    && isRecord(value.parameters)
    && isNullableString(value.user_label);
}

function isDynamicMemberLocator(value: unknown): boolean {
  if (!isRecord(value)) return false;
  if (value.kind === 'function_parameter') {
    return hasExactKeys(value, ['kind', 'function', 'parameter'])
      && typeof value.function === 'string'
      && typeof value.parameter === 'string';
  }
  return value.kind === 'schema_field'
    && hasExactKeys(value, ['kind', 'source', 'field'])
    && typeof value.source === 'string'
    && typeof value.field === 'string';
}

export function isTypeExprWire(value: unknown): value is TypeExprDto {
  if (value === 'Unknown') return true;
  if (!isRecord(value) || Object.keys(value).length !== 1) return false;
  if (hasExactKeys(value, ['Concrete'])) return typeof value.Concrete === 'string';
  if (hasExactKeys(value, ['Generic'])) return typeof value.Generic === 'string';
  if (hasExactKeys(value, ['Applied'])) {
    return isRecord(value.Applied)
      && hasExactKeys(value.Applied, ['constructor', 'arguments'])
      && typeof value.Applied.constructor === 'string'
      && Array.isArray(value.Applied.arguments)
      && value.Applied.arguments.every(isTypeExprWire);
  }
  return hasExactKeys(value, ['Union'])
    && Array.isArray(value.Union)
    && value.Union.every(isTypeExprWire);
}

function isLastKnownPortMetadata(value: unknown): boolean {
  if (!isRecord(value) || typeof value.label !== 'string') return false;
  return hasExactKeys(value, ['label'])
    || (hasExactKeys(value, ['label', 'value_type']) && isTypeExprWire(value.value_type));
}

function isDynamicPortBinding(value: unknown): boolean {
  if (!isRecord(value) || typeof value.order !== 'string') return false;
  if (value.kind === 'user_created') return hasExactKeys(value, ['kind', 'order']);
  if (value.kind === 'resolved') {
    return hasExactKeys(value, ['kind', 'origin', 'order', 'last_known'])
      && isDynamicMemberLocator(value.origin)
      && isLastKnownPortMetadata(value.last_known);
  }
  return value.kind === 'orphan'
    && hasExactKeys(value, ['kind', 'origin', 'order', 'last_known'])
    && isDynamicMemberLocator(value.origin)
    && isLastKnownPortMetadata(value.last_known);
}

function isDocumentConnection(value: unknown): boolean {
  return isRecord(value)
    && hasExactKeys(value, ['id', 'output', 'input', 'order'])
    && isUuid(value.id)
    && isDocumentPortAddress(value.output)
    && isDocumentPortAddress(value.input)
    && isNullableString(value.order);
}

function isInputState(value: unknown): boolean {
  return isRecord(value) && hasExactKeys(value, ['literal_override']);
}

function isGraphOperation(value: unknown): boolean {
  if (!isRecord(value) || typeof value.operation !== 'string') return false;
  switch (value.operation) {
    case 'insert_node':
    case 'remove_node':
      return hasExactKeys(value, ['operation', 'node']) && isDocumentNode(value.node);
    case 'update_node':
      return hasExactKeys(value, ['operation', 'before', 'after'])
        && isDocumentNode(value.before)
        && isDocumentNode(value.after);
    case 'insert_port_binding':
    case 'remove_port_binding':
      return hasExactKeys(value, ['operation', 'address', 'binding'])
        && isDocumentPortAddress(value.address)
        && isDynamicPortBinding(value.binding);
    case 'insert_connection':
    case 'remove_connection':
      return hasExactKeys(value, ['operation', 'connection'])
        && isDocumentConnection(value.connection);
    case 'set_input_state':
      return hasExactKeys(value, ['operation', 'address', 'before', 'after'])
        && isDocumentPortAddress(value.address)
        && (value.before === null || isInputState(value.before))
        && (value.after === null || isInputState(value.after));
    default:
      return false;
  }
}

function parseGraphPatch(value: unknown): GraphDocumentPatchDto {
  if (!isRecord(value)
    || !hasExactKeys(value, ['operations'])
    || !Array.isArray(value.operations)
    || !value.operations.every(isGraphOperation)) {
    throw new Error('GraphDelta graph patch operation is malformed');
  }
  return { operations: value.operations } as GraphDocumentPatchDto;
}

function isDiagnosticLocation(value: unknown): value is DiagnosticLocationDto {
  if (!isRecord(value)) return false;
  switch (value.kind) {
    case 'graph': return hasExactKeys(value, ['kind']);
    case 'node': return hasExactKeys(value, ['kind', 'nodeId']) && typeof value.nodeId === 'string';
    case 'port': return hasExactKeys(value, ['kind', 'address']) && isPortAddressDto(value.address);
    case 'connection':
      return hasExactKeys(value, ['kind', 'connectionId']) && typeof value.connectionId === 'string';
    case 'parameter':
      return hasExactKeys(value, ['kind', 'nodeId', 'key'])
        && typeof value.nodeId === 'string'
        && typeof value.key === 'string';
    case 'resource':
      return hasExactKeys(value, ['kind', 'identity']) && typeof value.identity === 'string';
    default: return false;
  }
}

function isDiagnostic(value: unknown): boolean {
  return isRecord(value)
    && hasExactKeys(value, ['code', 'message', 'severity', 'blocking', 'location', 'related'])
    && typeof value.code === 'string'
    && typeof value.message === 'string'
    && ['error', 'warning', 'information'].includes(value.severity as string)
    && typeof value.blocking === 'boolean'
    && isDiagnosticLocation(value.location)
    && Array.isArray(value.related)
    && value.related.every(isDiagnosticLocation);
}

function isProjectionBasis(value: unknown): boolean {
  return isRecord(value)
    && hasExactKeys(value, ['graphPath', 'graphRevision', 'registryFingerprint', 'resourceVersions'])
    && isGraphResourcePath(value.graphPath)
    && isSafeRevision(value.graphRevision)
    && typeof value.registryFingerprint === 'string'
    && FINGERPRINT_PATTERN.test(value.registryFingerprint)
    && isRecord(value.resourceVersions)
    && Object.values(value.resourceVersions).every((entry) => typeof entry === 'string');
}

function isPortConnections(value: unknown): boolean {
  return isRecord(value)
    && hasExactKeys(value, [
      'current', 'maximum', 'ordered', 'canAppend', 'canReplace', 'canMove',
    ])
    && isSafeRevision(value.current)
    && (value.maximum === null || isSafeRevision(value.maximum))
    && typeof value.ordered === 'boolean'
    && typeof value.canAppend === 'boolean'
    && typeof value.canReplace === 'boolean'
    && typeof value.canMove === 'boolean';
}

function isResolvedType(value: unknown): boolean {
  return value === null || (isRecord(value)
    && hasExactKeys(value, ['display', 'resolved', 'dataType'])
    && typeof value.display === 'string'
    && typeof value.resolved === 'boolean'
    && (value.dataType === null || isDataTypeBackendFormat(value.dataType)));
}

function isResolvedSchema(value: unknown): boolean {
  if (value === null) return true;
  return isRecord(value)
    && hasExactKeys(value, ['kind', 'fields'])
    && ['input', 'project', 'append', 'rename', 'filter', 'derived'].includes(value.kind as string)
    && Array.isArray(value.fields)
    && value.fields.every((field) => isRecord(field)
      && hasExactKeys(field, ['name', 'scalarType'])
      && typeof field.name === 'string'
      && ['boolean', 'int64', 'float64', 'string', 'date', 'dateTime', 'unknown']
        .includes(field.scalarType as string));
}

function isResolvedPort(value: unknown): boolean {
  if (!isRecord(value)
    || !hasExactKeys(value, [
      'address', 'templateKey', 'display', 'direction', 'kind', 'instanceKind', 'orphan',
      'canRemove', 'connections', 'input', 'resolvedType', 'resolvedSchema', 'status',
    ])) return false;
  return isPortAddressDto(value.address)
    && typeof value.templateKey === 'string'
    && isRecord(value.display)
    && hasExactKeys(value.display, ['label', 'instanceLabel'])
    && typeof value.display.label === 'string'
    && isNullableString(value.display.instanceLabel)
    && ['input', 'output'].includes(value.direction as string)
    && ['data', 'control', 'effect'].includes(value.kind as string)
    && ['declared', 'userCreated', 'derived'].includes(value.instanceKind as string)
    && typeof value.orphan === 'boolean'
    && typeof value.canRemove === 'boolean'
    && isPortConnections(value.connections)
    && (value.input === null || (isRecord(value.input)
      && hasExactKeys(value.input, ['literalOverride', 'protocolDefault', 'effective'])
      && ['connections', 'literal', 'protocolDefault', 'unbound'].includes(value.input.effective as string)))
    && isResolvedType(value.resolvedType)
    && isResolvedSchema(value.resolvedSchema)
    && ['resolved', 'orphan'].includes(value.status as string);
}

function isParameterConfiguration(value: unknown): boolean {
  if (value === null) return true;
  if (!isRecord(value)) return false;
  if (value.kind === 'projectColumns') {
    return hasExactKeys(value, ['kind', 'available', 'unavailableReason', 'options', 'value'])
      && typeof value.available === 'boolean'
      && isNullableString(value.unavailableReason)
      && Array.isArray(value.options)
      && Array.isArray(value.value)
      && value.value.every((entry) => typeof entry === 'string');
  }
  return value.kind === 'filterPredicate'
    && hasExactKeys(value, ['kind', 'available', 'unavailableReason', 'columns', 'value'])
    && typeof value.available === 'boolean'
    && isNullableString(value.unavailableReason)
    && Array.isArray(value.columns)
    && (value.value === null || isRecord(value.value));
}

function isParameterEditor(value: unknown): boolean {
  return isRecord(value)
    && hasExactKeys(value, [
      'key', 'display', 'editor', 'presentation', 'valueType', 'multiline', 'value', 'configuration',
      'inheritedValue', 'valueSource', 'options',
    ])
    && typeof value.key === 'string'
    && isRecord(value.display)
    && hasExactKeys(value.display, ['title', 'description'])
    && typeof value.display.title === 'string'
    && isNullableString(value.display.description)
    && ['auto', 'text', 'number', 'toggle', 'select', 'resource'].includes(value.editor as string)
    && ['detailPanel', 'inlineAndDetail'].includes(value.presentation as string)
    && (value.valueType === null || isDataTypeBackendFormat(value.valueType))
    && typeof value.multiline === 'boolean'
    && isJsonValue(value.value)
    && isParameterConfiguration(value.configuration)
    && isJsonValue(value.inheritedValue)
    && (value.valueSource === null || value.valueSource === 'project' || value.valueSource === 'node')
    && (value.options === null || (Array.isArray(value.options)
      && value.options.every((option) => typeof option === 'string')));
}

function isEditorNode(value: unknown): boolean {
  return isRecord(value)
    && hasExactKeys(value, [
      'graphPath', 'sourceRevision', 'nodeId', 'nodeTypeId', 'position', 'display', 'ports',
      'parameterEditors', 'capabilities', 'diagnostics',
    ])
    && isGraphResourcePath(value.graphPath)
    && isSafeRevision(value.sourceRevision)
    && typeof value.nodeId === 'string'
    && typeof value.nodeTypeId === 'string'
    && isPosition(value.position)
    && isRecord(value.display)
    && hasExactKeys(value.display, ['title', 'userLabel', 'iconId', 'styleId'])
    && typeof value.display.title === 'string'
    && isNullableString(value.display.userLabel)
    && isNullableString(value.display.iconId)
    && isNullableString(value.display.styleId)
    && Array.isArray(value.ports)
    && value.ports.every(isResolvedPort)
    && Array.isArray(value.parameterEditors)
    && value.parameterEditors.every(isParameterEditor)
    && isRecord(value.capabilities)
    && hasExactKeys(value.capabilities, [
      'managed', 'canCopy', 'canDelete', 'canEditLabel', 'canEditParameters',
      'hasDynamicPorts', 'supportsInlineLiterals',
    ])
    && Object.values(value.capabilities).every((entry) => typeof entry === 'boolean')
    && Array.isArray(value.diagnostics)
    && value.diagnostics.every(isDiagnostic);
}

function isEditorConnection(value: unknown): boolean {
  return isRecord(value)
    && hasExactKeys(value, ['connectionId', 'output', 'input', 'order'])
    && typeof value.connectionId === 'string'
    && isPortAddressDto(value.output)
    && isPortAddressDto(value.input)
    && isNullableString(value.order);
}

function isCompilationOutcome(value: unknown): boolean {
  if (!isRecord(value) || typeof value.type !== 'string') return false;
  if (value.type === 'success' || value.type === 'analysisBlocked') {
    return hasExactKeys(value, ['type']);
  }
  return value.type === 'internalFailure'
    && hasExactKeys(value, ['type', 'stage', 'code', 'nodeId'])
    && (value.stage === 'analysis' || value.stage === 'lowering')
    && typeof value.code === 'string'
    && value.code.length > 0
    && (value.nodeId === null || (typeof value.nodeId === 'string' && isUuid(value.nodeId)));
}

function isEditorProjection(value: unknown): value is EditorGraphProjectionDto {
  return isRecord(value)
    && hasExactKeys(value, [
      'basis', 'graphPath', 'sourceRevision', 'nodes', 'connections', 'diagnostics',
      'outcome', 'hasBlockingDiagnostics',
    ])
    && isProjectionBasis(value.basis)
    && isGraphResourcePath(value.graphPath)
    && isSafeRevision(value.sourceRevision)
    && Array.isArray(value.nodes)
    && value.nodes.every(isEditorNode)
    && Array.isArray(value.connections)
    && value.connections.every(isEditorConnection)
    && Array.isArray(value.diagnostics)
    && value.diagnostics.every(isDiagnostic)
    && isCompilationOutcome(value.outcome)
    && typeof value.hasBlockingDiagnostics === 'boolean'
    && ((value.outcome as { type: string }).type === 'success'
      ? value.hasBlockingDiagnostics === false
      : value.hasBlockingDiagnostics === true);
}

export function parseGraphProjectionReplacementDto(
  value: unknown,
): GraphProjectionReplacementDto {
  if (!isRecord(value)
    || !isGraphResourcePath(value.graphPath)
    || !isEditorProjection(value.projection)
    || value.projection.graphPath !== value.graphPath
    || value.projection.basis.graphPath !== value.graphPath
    || value.projection.sourceRevision !== value.projection.basis.graphRevision) {
    throw new Error('Graph mutation projection replacement is malformed');
  }
  if (value.graphPath.startsWith('events/')) {
    if (!hasExactKeys(value, ['graphPath', 'projection'])) {
      throw new Error('Graph mutation projection replacement is malformed');
    }
    return { graphPath: value.graphPath, projection: value.projection };
  }
  if (!hasExactKeys(value, ['graphPath', 'projection', 'functionEditorProjection'])
    || !isFunctionEditorProjectionDto(value.functionEditorProjection)) {
    throw new Error('Graph mutation projection replacement is malformed');
  }
  return {
    graphPath: value.graphPath,
    projection: value.projection,
    functionEditorProjection: value.functionEditorProjection,
  };
}

export function parseHistoryStatusDto(value: unknown): HistoryStatusDto {
  if (!isRecord(value)
    || !hasExactKeys(value, ['canUndo', 'canRedo'])
    || typeof value.canUndo !== 'boolean'
    || typeof value.canRedo !== 'boolean') {
    throw new Error('Graph mutation history is malformed');
  }
  return { canUndo: value.canUndo, canRedo: value.canRedo };
}

export function parseGraphDeltaDto(value: unknown): GraphDeltaDto {
  if (!isRecord(value)
    || !hasExactKeys(value, ['graphPath', 'fromRevision', 'toRevision', 'causedBy', 'payload'])) {
    throw new Error('GraphDelta must have exact graphPath, revision, causedBy, and payload fields');
  }
  if (!isGraphResourcePath(value.graphPath)) throw new Error('GraphDelta graphPath is malformed');
  const payload = parseGraphPatch(value.payload);
  if (!isSafeRevision(value.fromRevision)
    || !isSafeRevision(value.toRevision)
    || (payload.operations.length === 0
      ? value.toRevision !== value.fromRevision
      : value.toRevision !== value.fromRevision + 1)) {
    throw new Error('GraphDelta revision is malformed');
  }
  if (value.causedBy !== null && !isUuid(value.causedBy)) {
    throw new Error('GraphDelta causedBy is malformed');
  }
  return {
    graphPath: value.graphPath,
    fromRevision: value.fromRevision,
    toRevision: value.toRevision,
    causedBy: value.causedBy,
    payload,
  };
}

export function parseGraphMutationResultDto(
  value: unknown,
  expectedProjectInstanceId: string,
): GraphMutationResultDto {
  if (!isRecord(value)
    || !hasExactKeys(value, ['projectInstanceId', 'delta', 'projectionReplacement', 'history'])) {
    throw new Error('Graph mutation result must have exact projectInstanceId, delta, projectionReplacement, and history fields');
  }
  if (typeof value.projectInstanceId !== 'string'
    || value.projectInstanceId.length === 0
    || value.projectInstanceId !== expectedProjectInstanceId) {
    throw new Error('Graph mutation result projectInstanceId is malformed or mismatched');
  }
  const delta = parseGraphDeltaDto(value.delta);
  const projectionReplacement = parseGraphProjectionReplacementDto(value.projectionReplacement);
  if (projectionReplacement.graphPath !== delta.graphPath
    || projectionReplacement.projection.sourceRevision !== delta.toRevision) {
    throw new Error('Graph mutation projection replacement disagrees with delta');
  }
  return {
    projectInstanceId: value.projectInstanceId,
    delta,
    projectionReplacement,
    history: parseHistoryStatusDto(value.history),
  };
}
