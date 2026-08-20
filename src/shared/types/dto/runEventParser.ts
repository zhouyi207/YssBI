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
import type { ResultStateKind } from './result';
import {
  RUN_ERROR_CODES,
  RUN_EVENT_KIND_TYPES,
  RUN_OUTPUT_STATUSES,
  RUN_OUTPUT_STREAMS,
  RUN_PHASES,
  type CompilationBasisDto,
  type ExecuteGraphResultDto,
  type ExecutionChannelEvent,
  type RunCorrelationDto,
  type RunErrorCode,
  type RunEvent,
  type RunEventKind,
  type RunOutputChannelEvent,
  type RunPhase,
} from './runEvent';

type UnknownRecord = Record<string, unknown>;

const DECIMAL_ID_PATTERN = /^(0|[1-9]\d*)$/;
const POSITIVE_DECIMAL_ID_PATTERN = /^[1-9]\d*$/;
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

function isGeneration(value: unknown): value is number {
  return Number.isSafeInteger(value) && (value as number) >= 0;
}

function isDecimalId(value: unknown): value is string {
  return typeof value === 'string' && DECIMAL_ID_PATTERN.test(value);
}

function isPositiveDecimalId(value: unknown): value is string {
  return typeof value === 'string' && POSITIVE_DECIMAL_ID_PATTERN.test(value);
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
    case 'pinPreview':
      if (!hasExactKeys(value, ['type', 'output', 'generation'])
        || !isGeneration(value.generation)) return fail('pin preview execution demand');
      return {
        type: 'pinPreview',
        output: parseGraphOutputRefDto(value.output),
        generation: value.generation,
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
    case 'activationIdExhausted':
    case 'runtimeIdExhausted':
    case 'deadlineExceeded':
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

function parseRunPhase(value: unknown): RunPhase {
  return parseDiscriminant(value, RUN_PHASES, 'run phase');
}

function parseErrorOutcome(
  value: UnknownRecord,
): { code: 'deadlineExceeded'; phase: RunPhase }
  | { code: Exclude<RunErrorCode, 'deadlineExceeded'>; phase: null } {
  const code = parseRunErrorCode(value.code);
  if (code === 'deadlineExceeded') {
    return { code, phase: parseRunPhase(value.phase) };
  }
  if (value.phase !== null) return fail('run error phase');
  return { code, phase: null };
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
    || !(value.runId === null || isPositiveDecimalId(value.runId))
    || !(value.nodeId === null || isUuid(value.nodeId))
    || !isNullableString(value.nodeTypeId)
    || !(value.parentCall === null || isPositiveDecimalId(value.parentCall))) {
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
  if (!hasExactKeys(value, ['type', 'operationIndex', 'activationId', 'attemptId'])
    || !isU32(value.operationIndex)
    || !isPositiveDecimalId(value.activationId)
    || !isPositiveDecimalId(value.attemptId)) return fail(type);
  return {
    type,
    operationIndex: value.operationIndex,
    activationId: value.activationId,
    attemptId: value.attemptId,
  };
}

function parseResultStateKind(value: unknown): ResultStateKind {
  if (value === 'pending' || value === 'ready' || value === 'failed' || value === 'cancelled') {
    return value;
  }
  return fail('result state kind');
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
      if (!hasExactKeys(value, ['type', 'code', 'phase'])) return fail('runErrored');
      return { type: 'runErrored', ...parseErrorOutcome(value) };
    case 'runCancelled':
      if (!hasExactKeys(value, ['type'])) return fail('runCancelled');
      return { type: 'runCancelled' };
    case 'operationStarted':
      return parseOperationEvent(value, 'operationStarted');
    case 'operationCompleted':
      return parseOperationEvent(value, 'operationCompleted');
    case 'operationErrored':
      if (!hasExactKeys(
        value,
        ['type', 'operationIndex', 'activationId', 'attemptId', 'code', 'phase'],
      )
        || !isU32(value.operationIndex)
        || !isPositiveDecimalId(value.activationId)
        || !isPositiveDecimalId(value.attemptId)) return fail('operationErrored');
      return {
        type: 'operationErrored',
        operationIndex: value.operationIndex,
        activationId: value.activationId,
        attemptId: value.attemptId,
        ...parseErrorOutcome(value),
      };
    case 'resultGroupChanged':
      if (!hasExactKeys(value, ['type', 'activationId', 'resultIds', 'state'])
        || !isDecimalId(value.activationId)
        || !Array.isArray(value.resultIds)
        || !value.resultIds.every(isPositiveDecimalId)) return fail('resultGroupChanged');
      return {
        type: 'resultGroupChanged',
        activationId: value.activationId,
        resultIds: value.resultIds,
        state: parseResultStateKind(value.state),
      };
    case 'outputResultChanged':
      if (!hasExactKeys(value, ['type', 'output', 'generation', 'resultId'])
        || !(value.generation === null || isGeneration(value.generation))
        || !isPositiveDecimalId(value.resultId)) return fail('outputResultChanged');
      return {
        type: 'outputResultChanged',
        output: parseGraphOutputRefDto(value.output),
        generation: value.generation,
        resultId: value.resultId,
      };
    case 'openResultWindow':
      if (!hasExactKeys(value, ['type', 'resultId']) || !isPositiveDecimalId(value.resultId)) {
        return fail('openResultWindow');
      }
      return { type: 'openResultWindow', resultId: value.resultId };
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

export function parseRunOutputChannelEvent(value: unknown): RunOutputChannelEvent {
  if (!isRecord(value)
    || !isPositiveDecimalId(value.runId)
    || !Number.isSafeInteger(value.sequence)
    || (value.sequence as number) < 1
    || !isGraphResourcePath(value.sourceGraphPath)
    || !isUuid(value.sourceNodeId)) {
    return fail('run output event');
  }
  const stream = parseDiscriminant(value.stream, RUN_OUTPUT_STREAMS, 'run output stream');
  if (Object.prototype.hasOwnProperty.call(value, 'text')) {
    if (!hasExactKeys(value, [
      'runId', 'sequence', 'stream', 'text', 'sourceGraphPath', 'sourceNodeId',
    ]) || typeof value.text !== 'string') return fail('run output event');
    return {
      runId: value.runId,
      sequence: value.sequence as number,
      stream,
      text: value.text,
      sourceGraphPath: value.sourceGraphPath,
      sourceNodeId: value.sourceNodeId,
    };
  }
  if (!hasExactKeys(value, [
    'runId', 'sequence', 'stream', 'status', 'sourceGraphPath', 'sourceNodeId',
  ])) {
    return fail('run output status event');
  }
  return {
    runId: value.runId,
    sequence: value.sequence as number,
    stream,
    status: parseDiscriminant(value.status, RUN_OUTPUT_STATUSES, 'run output status'),
    sourceGraphPath: value.sourceGraphPath,
    sourceNodeId: value.sourceNodeId,
  };
}

export function parseExecutionChannelEvent(value: unknown): ExecutionChannelEvent {
  if (isRecord(value) && Object.prototype.hasOwnProperty.call(value, 'kind')) {
    return parseRunEvent(value);
  }
  return parseRunOutputChannelEvent(value);
}

export function parseExecuteGraphResultDto(value: unknown): ExecuteGraphResultDto {
  if (!isRecord(value)
    || !hasExactKeys(value, ['runId'])
    || !isPositiveDecimalId(value.runId)) {
    return fail('execute graph result');
  }
  return { runId: value.runId };
}
