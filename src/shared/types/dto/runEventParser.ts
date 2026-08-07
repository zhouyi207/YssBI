import {
  EXECUTION_DEMAND_TYPES,
  type ExecutionDemandDto,
  type GraphOutputRefDto,
} from './executionDemand';
import {
  isGraphResourcePath,
  isPortAddressDto,
  isUuid,
} from './editorProjectionGuards';
import {
  RUN_ERROR_CODES,
  RUN_EVENT_KIND_TYPES,
  type CompilationBasisDto,
  type ExecuteGraphResultDto,
  type RunCorrelationDto,
  type RunErrorCode,
  type RunEvent,
  type RunEventKind,
} from './runEvent';

type UnknownRecord = Record<string, unknown>;

const DECIMAL_ID_PATTERN = /^(0|[1-9]\d*)$/;
const FINGERPRINT_PATTERN = /^[0-9a-f]{64}$/;
const MAX_U32 = 4_294_967_295;

function isRecord(value: unknown): value is UnknownRecord {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function hasExactKeys(value: UnknownRecord, keys: readonly string[]): boolean {
  return Object.keys(value).length === keys.length
    && keys.every((key) => Object.prototype.hasOwnProperty.call(value, key));
}

function fail(contract: string): never {
  throw new Error(`Invalid ${contract}`);
}

function assertNever(value: never): never {
  return fail(`unhandled discriminant ${String(value)}`);
}

function parseDiscriminant<T extends string>(
  value: unknown,
  inventory: Readonly<Record<T, true>>,
  contract: string,
): T {
  if (typeof value !== 'string'
    || !Object.prototype.hasOwnProperty.call(inventory, value)) return fail(contract);
  return value as T;
}

function isU32(value: unknown): value is number {
  return Number.isSafeInteger(value) && (value as number) >= 0 && (value as number) <= MAX_U32;
}

function isDecimalId(value: unknown): value is string {
  return typeof value === 'string' && DECIMAL_ID_PATTERN.test(value);
}

function isNullableString(value: unknown): value is string | null {
  return value === null || typeof value === 'string';
}

function parseResourceVersions(value: unknown): Record<string, string> {
  if (!isRecord(value) || !Object.values(value).every((entry) => typeof entry === 'string')) {
    return fail('resource version set');
  }
  return value as Record<string, string>;
}

function parseGraphOutputRefDto(value: unknown): GraphOutputRefDto {
  if (!isRecord(value)
    || !hasExactKeys(value, ['graphPath', 'port'])
    || !isGraphResourcePath(value.graphPath)
    || !isPortAddressDto(value.port)) {
    return fail('graph output reference');
  }
  return { graphPath: value.graphPath, port: value.port };
}

export function parseExecutionDemandDto(value: unknown): ExecutionDemandDto {
  if (!isRecord(value)) return fail('execution demand');
  const type = parseDiscriminant(value.type, EXECUTION_DEMAND_TYPES, 'execution demand variant');
  switch (type) {
    case 'default':
      if (!hasExactKeys(value, ['type'])) return fail('default execution demand');
      return { type: 'default' };
    case 'outputs':
      if (!hasExactKeys(value, ['type', 'outputs', 'includeDefaultResults'])
        || !Array.isArray(value.outputs)
        || typeof value.includeDefaultResults !== 'boolean') {
        return fail('outputs execution demand');
      }
      return {
        type: 'outputs',
        outputs: value.outputs.map(parseGraphOutputRefDto),
        includeDefaultResults: value.includeDefaultResults,
      };
    default:
      return assertNever(type);
  }
}

function parseRunErrorCode(value: unknown): RunErrorCode {
  const code = parseDiscriminant(value, RUN_ERROR_CODES, 'run error code');
  switch (code) {
    case 'invalidPlan':
    case 'cancelled':
    case 'kernelNotFound':
    case 'kernelFailed':
    case 'relationalBackendNotFound':
    case 'relationalOperatorInvalid':
    case 'relationalColumnMissing':
    case 'relationalTypeMismatch':
    case 'relationalInputShapeInvalid':
    case 'relationalHintInvalid':
    case 'missingRelationalFragment':
    case 'bridgeFailed':
    case 'stream':
    case 'missingValue':
    case 'invalidCondition':
    case 'outputCount':
    case 'operationAlreadyExecuted':
    case 'unsatisfiedEffectDependency':
    case 'loopLimitExceeded':
    case 'functionPlanNotFound':
    case 'functionPlanFailed':
    case 'recursionLimitExceeded':
    case 'projectDraining':
    case 'resourceSnapshotMismatch':
    case 'resourceAcquire':
      return code;
    default:
      return assertNever(code);
  }
}

function parseCompilationBasisDto(value: unknown): CompilationBasisDto {
  if (!isRecord(value)
    || !hasExactKeys(value, ['graphRevision', 'registryFingerprint', 'resourceVersions'])
    || !isDecimalId(value.graphRevision)
    || typeof value.registryFingerprint !== 'string'
    || !FINGERPRINT_PATTERN.test(value.registryFingerprint)) {
    return fail('compilation basis');
  }
  return {
    graphRevision: value.graphRevision,
    registryFingerprint: value.registryFingerprint,
    resourceVersions: parseResourceVersions(value.resourceVersions),
  };
}

function parseRunCorrelationDto(value: unknown): RunCorrelationDto {
  if (!isRecord(value)
    || !hasExactKeys(value, [
      'projectSessionId', 'graphPath', 'graphRevision', 'registryFingerprint', 'resourceVersions',
      'compileId', 'selectionDigest', 'runId', 'nodeId', 'nodeTypeId', 'parentCall',
    ])
    || typeof value.projectSessionId !== 'string'
    || !isGraphResourcePath(value.graphPath)
    || !isDecimalId(value.graphRevision)
    || typeof value.registryFingerprint !== 'string'
    || !FINGERPRINT_PATTERN.test(value.registryFingerprint)
    || !isDecimalId(value.compileId)
    || !isNullableString(value.selectionDigest)
    || !(value.runId === null || isDecimalId(value.runId))
    || !(value.nodeId === null || isUuid(value.nodeId))
    || !isNullableString(value.nodeTypeId)
    || !(value.parentCall === null || isDecimalId(value.parentCall))) {
    return fail('run correlation');
  }
  return {
    projectSessionId: value.projectSessionId,
    graphPath: value.graphPath,
    graphRevision: value.graphRevision,
    registryFingerprint: value.registryFingerprint,
    resourceVersions: parseResourceVersions(value.resourceVersions),
    compileId: value.compileId,
    selectionDigest: value.selectionDigest,
    runId: value.runId,
    nodeId: value.nodeId,
    nodeTypeId: value.nodeTypeId,
    parentCall: value.parentCall,
  };
}

function parseOperationEvent(
  value: UnknownRecord,
  type: 'operationStarted' | 'operationCompleted',
): RunEventKind {
  if (!hasExactKeys(value, ['type', 'operationIndex', 'activationId'])
    || !isU32(value.operationIndex)
    || !isDecimalId(value.activationId)) return fail(type);
  return { type, operationIndex: value.operationIndex, activationId: value.activationId };
}

function parseRunEventKind(value: unknown): RunEventKind {
  if (!isRecord(value)) return fail('run event kind');
  const type = parseDiscriminant(value.type, RUN_EVENT_KIND_TYPES, 'run event kind variant');
  switch (type) {
    case 'runStarted':
      if (!hasExactKeys(value, ['type'])) return fail('runStarted');
      return { type: 'runStarted' };
    case 'runCompleted':
      if (!hasExactKeys(value, ['type'])) return fail('runCompleted');
      return { type: 'runCompleted' };
    case 'runErrored':
      if (!hasExactKeys(value, ['type', 'code'])) return fail('runErrored');
      return { type: 'runErrored', code: parseRunErrorCode(value.code) };
    case 'runCancelled':
      if (!hasExactKeys(value, ['type'])) return fail('runCancelled');
      return { type: 'runCancelled' };
    case 'operationStarted':
      return parseOperationEvent(value, 'operationStarted');
    case 'operationCompleted':
      return parseOperationEvent(value, 'operationCompleted');
    case 'operationErrored':
      if (!hasExactKeys(value, ['type', 'operationIndex', 'activationId', 'code'])
        || !isU32(value.operationIndex)
        || !isDecimalId(value.activationId)) return fail('operationErrored');
      return {
        type: 'operationErrored',
        operationIndex: value.operationIndex,
        activationId: value.activationId,
        code: parseRunErrorCode(value.code),
      };
    case 'valueReady':
      if (!hasExactKeys(value, ['type', 'valueIndex', 'sourceId'])
        || !isU32(value.valueIndex)
        || !isDecimalId(value.sourceId)) return fail('valueReady');
      return { type: 'valueReady', valueIndex: value.valueIndex, sourceId: value.sourceId };
    case 'resultReady':
      if (!hasExactKeys(value, ['type', 'name', 'sourceId'])
        || typeof value.name !== 'string'
        || !isDecimalId(value.sourceId)) return fail('resultReady');
      return { type: 'resultReady', name: value.name, sourceId: value.sourceId };
    case 'outputReady':
      if (!hasExactKeys(value, ['type', 'output', 'sourceId'])
        || !isDecimalId(value.sourceId)) return fail('outputReady');
      return {
        type: 'outputReady',
        output: parseGraphOutputRefDto(value.output),
        sourceId: value.sourceId,
      };
    default:
      return assertNever(type);
  }
}

export function parseRunEvent(value: unknown): RunEvent {
  if (!isRecord(value) || !hasExactKeys(value, ['correlation', 'basis', 'kind'])) {
    return fail('run event');
  }
  return {
    correlation: parseRunCorrelationDto(value.correlation),
    basis: parseCompilationBasisDto(value.basis),
    kind: parseRunEventKind(value.kind),
  };
}

export function parseExecuteGraphResultDto(value: unknown): ExecuteGraphResultDto {
  if (!isRecord(value) || !hasExactKeys(value, ['runId']) || !isDecimalId(value.runId)) {
    return fail('execute graph result');
  }
  return { runId: value.runId };
}
