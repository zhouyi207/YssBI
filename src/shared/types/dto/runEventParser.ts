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
  RUN_OUTPUT_STATUSES,
  RUN_OUTPUT_STREAMS,
  RUN_PHASES,
  type ExecutionChannelEvent,
  type GraphRunIdentityDto,
  type RunErrorCode,
  type RunErrorOutcome,
  type RunEvent,
  type RunEventKind,
  type RunOutputChannelEvent,
  type RunPhase,
} from './runEvent';

type UnknownRecord = Record<string, unknown>;

const POSITIVE_DECIMAL_ID_PATTERN = /^[1-9]\d*$/;

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

function isGeneration(value: unknown): value is number {
  return Number.isSafeInteger(value) && (value as number) >= 0;
}

function isPositiveDecimalId(value: unknown): value is string {
  return typeof value === 'string' && POSITIVE_DECIMAL_ID_PATTERN.test(value);
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

function parseErrorOutcome(value: UnknownRecord): RunErrorOutcome {
  const code = parseRunErrorCode(value.code);
  if (code === 'deadlineExceeded') {
    return { code, phase: parseRunPhase(value.phase) };
  }
  if (value.phase !== null) return fail('run error phase');
  return { code, phase: null };
}

function parseGraphRunIdentityDto(value: unknown): GraphRunIdentityDto {
  if (
    !isRecord(value)
    || !hasExactKeys(value, ['projectSessionId', 'graphPath', 'runId'])
    || typeof value.projectSessionId !== 'string'
    || value.projectSessionId.length === 0
    || !isGraphResourcePath(value.graphPath)
    || !isPositiveDecimalId(value.runId)
  ) return fail('graph run identity');

  return {
    projectSessionId: value.projectSessionId,
    graphPath: value.graphPath,
    runId: value.runId,
  };
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
    case 'pinPreviewResultReady':
      if (
        !hasExactKeys(value, ['type', 'output', 'generation', 'resultId'])
        || !isGeneration(value.generation)
        || !isPositiveDecimalId(value.resultId)
      ) return fail('pinPreviewResultReady');
      return {
        type: 'pinPreviewResultReady',
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
  if (!isRecord(value) || !hasExactKeys(value, ['run', 'kind'])) {
    return fail('run event');
  }
  return {
    run: parseGraphRunIdentityDto(value.run),
    kind: parseRunEventKind(value.kind),
  };
}

export function parseRunOutputChannelEvent(value: unknown): RunOutputChannelEvent {
  if (!isRecord(value)
    || !isPositiveDecimalId(value.runId)
    || !Number.isSafeInteger(value.sequence)
    || (value.sequence as number) < 1
    || !isGraphResourcePath(value.sourceGraphPath)
    || !isUuid(value.sourceNodeId)
    || !isPortAddressDto(value.sourcePort)
    || value.sourcePort.nodeId !== value.sourceNodeId) {
    return fail('run output event');
  }
  const stream = parseDiscriminant(value.stream, RUN_OUTPUT_STREAMS, 'run output stream');
  if (Object.prototype.hasOwnProperty.call(value, 'text')) {
    if (!hasExactKeys(value, [
      'runId', 'sequence', 'stream', 'text', 'sourceGraphPath', 'sourceNodeId', 'sourcePort',
    ]) || typeof value.text !== 'string') return fail('run output event');
    return {
      runId: value.runId,
      sequence: value.sequence as number,
      stream,
      text: value.text,
      sourceGraphPath: value.sourceGraphPath,
      sourceNodeId: value.sourceNodeId,
      sourcePort: value.sourcePort,
    };
  }
  if (!hasExactKeys(value, [
    'runId', 'sequence', 'stream', 'status', 'sourceGraphPath', 'sourceNodeId', 'sourcePort',
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
    sourcePort: value.sourcePort,
  };
}

export function parseExecutionChannelEvent(value: unknown): ExecutionChannelEvent {
  if (isRecord(value) && Object.prototype.hasOwnProperty.call(value, 'kind')) {
    return parseRunEvent(value);
  }
  return parseRunOutputChannelEvent(value);
}
