export type TraceDecimalString = `${bigint}`;

export type TraceKindDto =
  | 'snapshot'
  | 'analysis'
  | 'lowering'
  | 'run'
  | 'operationAttempt'
  | 'resourceAcquire'
  | 'adapterIo'
  | 'resultPublication'
  | 'cleanup';

export type TraceOutcomeDto =
  | 'success'
  | 'error'
  | 'cancellation'
  | 'timeout'
  | 'retry'
  | 'notReached'
  | 'internalAborted'
  | { cleanup: { errorCount: TraceDecimalString; panicking: boolean } };

export interface TraceCorrelationDto {
  projectSessionId: string;
  graphPath: string;
  graphRevision: TraceDecimalString;
  registryFingerprint: string;
  resourceVersions: Record<string, string>;
  compileId: TraceDecimalString;
  selectionDigest: string | null;
  runId: TraceDecimalString | null;
  nodeId: string | null;
  nodeTypeId: string | null;
  parentCall: TraceDecimalString | null;
}

export interface TraceSpanDto {
  spanId: TraceDecimalString;
  parentSpanId: TraceDecimalString | null;
  runId: TraceDecimalString | null;
  operationId: string | null;
  activationId: TraceDecimalString | null;
  attemptId: TraceDecimalString | null;
  kind: TraceKindDto;
  startedAt: TraceDecimalString;
  finishedAt: TraceDecimalString;
  outcome: TraceOutcomeDto;
  correlation: TraceCorrelationDto;
}

const SPAN_KEYS = [
  'spanId', 'parentSpanId', 'runId', 'operationId', 'activationId', 'attemptId',
  'kind', 'startedAt', 'finishedAt', 'outcome', 'correlation',
] as const;
const CORRELATION_KEYS = [
  'projectSessionId', 'graphPath', 'graphRevision', 'registryFingerprint',
  'resourceVersions', 'compileId', 'selectionDigest', 'runId', 'nodeId',
  'nodeTypeId', 'parentCall',
] as const;
const TRACE_KINDS = new Set<TraceKindDto>([
  'snapshot', 'analysis', 'lowering', 'run', 'operationAttempt',
  'resourceAcquire', 'adapterIo', 'resultPublication', 'cleanup',
]);
const SIMPLE_OUTCOMES = new Set([
  'success', 'error', 'cancellation', 'timeout', 'retry', 'notReached', 'internalAborted',
]);

export function parseTraceSpanList(value: unknown): TraceSpanDto[] {
  if (!Array.isArray(value)) throw invalidTrace();
  const spans = value.map(parseTraceSpan);
  const spanIds = new Set(spans.map((span) => span.spanId));
  if (spanIds.size !== spans.length) throw invalidTrace();
  if (spans.some((span) => span.parentSpanId !== null && !spanIds.has(span.parentSpanId))) {
    throw invalidTrace();
  }
  validateTraceHierarchy(spans);
  return spans;
}

function validateTraceHierarchy(spans: TraceSpanDto[]): void {
  const indices = new Map(spans.map((span, index) => [span.spanId, index]));
  const colors = new Uint8Array(spans.length);

  for (let index = 0; index < spans.length; index += 1) {
    if (colors[index] !== 0) continue;
    let current: number | undefined = index;
    const path: number[] = [];
    while (current !== undefined && colors[current] === 0) {
      colors[current] = 1;
      path.push(current);
      const parentId: TraceDecimalString | null = spans[current].parentSpanId;
      current = parentId === null ? undefined : indices.get(parentId);
    }
    if (current !== undefined && colors[current] === 1) throw invalidTrace();
    for (const visited of path) colors[visited] = 2;
  }

  for (const span of spans) {
    const parent = span.parentSpanId === null
      ? null
      : spans[indices.get(span.parentSpanId)!];
    if (!isCompatibleTraceParent(span, parent)) throw invalidTrace();
  }
}

function isCompatibleTraceParent(span: TraceSpanDto, parent: TraceSpanDto | null): boolean {
  if (!hasValidKindSemantics(span)) return false;
  if (parent !== null && !sameTraceLineage(span, parent)) return false;
  if (parent !== null && isRuntimeKind(span.kind) && !intervalContains(parent, span)) return false;
  switch (span.kind) {
    case 'snapshot':
      return parent === null;
    case 'analysis':
    case 'lowering':
      return parent?.kind === 'snapshot';
    case 'run':
      return parent === null || parent.kind === 'run';
    case 'resourceAcquire':
    case 'resultPublication':
    case 'cleanup':
    case 'operationAttempt':
      return parent?.kind === 'run';
    case 'adapterIo':
      return parent?.kind === 'operationAttempt'
        && span.operationId === parent.operationId
        && span.activationId === parent.activationId
        && span.attemptId === parent.attemptId;
  }
}

function hasValidKindSemantics(span: TraceSpanDto): boolean {
  const compiler = span.kind === 'snapshot' || span.kind === 'analysis' || span.kind === 'lowering';
  if (compiler) {
    return span.runId === null
      && hasNoOperationIdentity(span)
      && isGeneralOutcome(span.outcome);
  }
  if (span.runId === null) return false;
  switch (span.kind) {
    case 'run':
      return hasNoOperationIdentity(span) && isGeneralOutcome(span.outcome);
    case 'resourceAcquire':
    case 'resultPublication':
      return hasNoOperationIdentity(span) && isPhaseOutcome(span.outcome);
    case 'cleanup':
      return hasNoOperationIdentity(span)
        && (span.outcome === 'notReached'
          || span.outcome === 'internalAborted'
          || isCleanupOutcome(span.outcome));
    case 'operationAttempt':
      return hasOperationIdentity(span)
        && (isGeneralOutcome(span.outcome) || span.outcome === 'retry');
    case 'adapterIo':
      return hasOperationIdentity(span) && isGeneralOutcome(span.outcome);
    default:
      return false;
  }
}

function hasOperationIdentity(span: TraceSpanDto): boolean {
  return span.operationId !== null && span.activationId !== null && span.attemptId !== null;
}

function isGeneralOutcome(outcome: TraceOutcomeDto): boolean {
  return typeof outcome === 'string'
    && outcome !== 'retry'
    && outcome !== 'notReached';
}

function isPhaseOutcome(outcome: TraceOutcomeDto): boolean {
  return isGeneralOutcome(outcome) || outcome === 'notReached';
}

function isCleanupOutcome(outcome: TraceOutcomeDto): outcome is Extract<TraceOutcomeDto, object> {
  return typeof outcome === 'object';
}

function isRuntimeKind(kind: TraceKindDto): boolean {
  return kind !== 'snapshot' && kind !== 'analysis' && kind !== 'lowering';
}

function intervalContains(parent: TraceSpanDto, child: TraceSpanDto): boolean {
  return BigInt(child.startedAt) >= BigInt(parent.startedAt)
    && BigInt(child.finishedAt) <= BigInt(parent.finishedAt);
}

function hasNoOperationIdentity(span: TraceSpanDto): boolean {
  return span.operationId === null && span.activationId === null && span.attemptId === null;
}

function sameTraceLineage(span: TraceSpanDto, parent: TraceSpanDto): boolean {
  return span.runId === parent.runId
    && span.correlation.projectSessionId === parent.correlation.projectSessionId
    && span.correlation.graphPath === parent.correlation.graphPath
    && span.correlation.graphRevision === parent.correlation.graphRevision
    && span.correlation.registryFingerprint === parent.correlation.registryFingerprint
    && span.correlation.compileId === parent.correlation.compileId;
}

function parseTraceSpan(value: unknown): TraceSpanDto {
  if (!isExactRecord(value, SPAN_KEYS)) throw invalidTrace();
  const correlation = parseCorrelation(value.correlation);
  if (
    !isPositiveDecimal(value.spanId)
    || !isNullablePositiveDecimal(value.parentSpanId)
    || !isNullablePositiveDecimal(value.runId)
    || !isNullableString(value.operationId)
    || !isNullablePositiveDecimal(value.activationId)
    || !isNullablePositiveDecimal(value.attemptId)
    || typeof value.kind !== 'string'
    || !TRACE_KINDS.has(value.kind as TraceKindDto)
    || !isDecimal(value.startedAt)
    || !isDecimal(value.finishedAt)
    || BigInt(value.finishedAt) < BigInt(value.startedAt)
    || !isOutcome(value.outcome)
    || value.runId !== correlation.runId
  ) {
    throw invalidTrace();
  }
  return value as unknown as TraceSpanDto;
}

function parseCorrelation(value: unknown): TraceCorrelationDto {
  if (!isExactRecord(value, CORRELATION_KEYS)) throw invalidTrace();
  if (
    typeof value.projectSessionId !== 'string'
    || typeof value.graphPath !== 'string'
    || !isDecimal(value.graphRevision)
    || typeof value.registryFingerprint !== 'string'
    || !isStringRecord(value.resourceVersions)
    || !isDecimal(value.compileId)
    || !isNullableString(value.selectionDigest)
    || !isNullablePositiveDecimal(value.runId)
    || !isNullableString(value.nodeId)
    || !isNullableString(value.nodeTypeId)
    || !isNullablePositiveDecimal(value.parentCall)
  ) {
    throw invalidTrace();
  }
  return value as unknown as TraceCorrelationDto;
}

function isOutcome(value: unknown): value is TraceOutcomeDto {
  if (typeof value === 'string') return SIMPLE_OUTCOMES.has(value);
  if (!isExactRecord(value, ['cleanup'])) return false;
  return isExactRecord(value.cleanup, ['errorCount', 'panicking'])
    && isDecimal(value.cleanup.errorCount)
    && typeof value.cleanup.panicking === 'boolean';
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function isExactRecord(value: unknown, keys: readonly string[]): value is Record<string, unknown> {
  if (!isRecord(value)) return false;
  const actual = Object.keys(value).sort();
  const expected = [...keys].sort();
  return actual.length === expected.length && actual.every((key, index) => key === expected[index]);
}

function isStringRecord(value: unknown): value is Record<string, string> {
  return isRecord(value) && Object.values(value).every((entry) => typeof entry === 'string');
}

function isDecimal(value: unknown): value is TraceDecimalString {
  return typeof value === 'string' && /^(0|[1-9]\d*)$/.test(value);
}

function isPositiveDecimal(value: unknown): value is TraceDecimalString {
  return isDecimal(value) && value !== '0';
}

function isNullablePositiveDecimal(value: unknown): value is TraceDecimalString | null {
  return value === null || isPositiveDecimal(value);
}

function isNullableString(value: unknown): value is string | null {
  return value === null || typeof value === 'string';
}

function invalidTrace(): Error {
  return new Error('Invalid trace span response');
}
