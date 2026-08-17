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

export interface TraceProvenanceScopeDto {
  projectSessionId: string;
  graphPath: string;
  graphRevision: TraceDecimalString;
  registryFingerprint: string;
  resourceVersions: Record<string, string>;
  compileId: TraceDecimalString;
}

export interface TraceBundleMetadataDto {
  provenanceScopes: TraceProvenanceScopeDto[];
  truncated: boolean;
  droppedSpanCount: TraceDecimalString;
  estimatedBytes: TraceDecimalString;
}

export interface CompilationTraceBundleDto {
  bundleKind: 'compilation';
  compileId: TraceDecimalString;
  graphPath: string;
  metadata: TraceBundleMetadataDto;
  spans: TraceSpanDto[];
}

export interface RunTraceBundleDto {
  bundleKind: 'run';
  runId: TraceDecimalString;
  compileId: TraceDecimalString;
  graphPath: string;
  selectionDigest: string | null;
  incidentId: string | null;
  metadata: TraceBundleMetadataDto;
  spans: TraceSpanDto[];
}

export type TraceBundleDto = CompilationTraceBundleDto | RunTraceBundleDto;

const SPAN_KEYS = [
  'spanId', 'parentSpanId', 'runId', 'operationId', 'activationId', 'attemptId',
  'kind', 'startedAt', 'finishedAt', 'outcome', 'correlation',
] as const;
const CORRELATION_KEYS = [
  'projectSessionId', 'graphPath', 'graphRevision', 'registryFingerprint',
  'resourceVersions', 'compileId', 'selectionDigest', 'runId', 'nodeId',
  'nodeTypeId', 'parentCall',
] as const;
const PROVENANCE_SCOPE_KEYS = [
  'projectSessionId', 'graphPath', 'graphRevision', 'registryFingerprint',
  'resourceVersions', 'compileId',
] as const;
const METADATA_KEYS = [
  'provenanceScopes', 'truncated', 'droppedSpanCount', 'estimatedBytes',
] as const;
const COMPILATION_BUNDLE_KEYS = [
  'bundleKind', 'compileId', 'graphPath', 'metadata', 'spans',
] as const;
const RUN_BUNDLE_KEYS = [
  'bundleKind', 'runId', 'compileId', 'graphPath', 'selectionDigest',
  'incidentId', 'metadata', 'spans',
] as const;
const TRACE_KINDS = new Set<TraceKindDto>([
  'snapshot', 'analysis', 'lowering', 'run', 'operationAttempt',
  'resourceAcquire', 'adapterIo', 'resultPublication', 'cleanup',
]);
const SIMPLE_OUTCOMES = new Set([
  'success', 'error', 'cancellation', 'timeout', 'retry', 'notReached', 'internalAborted',
]);

export function parseTraceBundleList(value: unknown): TraceBundleDto[] {
  if (!Array.isArray(value)) throw invalidTrace();
  return value.map(parseTraceBundle);
}

export function parseRunTraceBundle(value: unknown): RunTraceBundleDto {
  const bundle = parseTraceBundle(value);
  if (bundle.bundleKind !== 'run') throw invalidTrace();
  return bundle;
}

function parseTraceBundle(value: unknown): TraceBundleDto {
  if (!isRecord(value) || typeof value.bundleKind !== 'string') throw invalidTrace();
  if (value.bundleKind === 'compilation') return parseCompilationBundle(value);
  if (value.bundleKind === 'run') return parseRunBundle(value);
  throw invalidTrace();
}

function parseCompilationBundle(value: Record<string, unknown>): CompilationTraceBundleDto {
  if (
    !hasExactKeys(value, COMPILATION_BUNDLE_KEYS)
    || !isDecimal(value.compileId)
    || typeof value.graphPath !== 'string'
  ) {
    throw invalidTrace();
  }
  const metadata = parseMetadata(value.metadata);
  const spans = parseSpans(value.spans);
  return {
    bundleKind: 'compilation',
    compileId: value.compileId,
    graphPath: value.graphPath,
    metadata,
    spans,
  };
}

function parseRunBundle(value: Record<string, unknown>): RunTraceBundleDto {
  if (
    !hasExactKeys(value, RUN_BUNDLE_KEYS)
    || !isPositiveDecimal(value.runId)
    || !isDecimal(value.compileId)
    || typeof value.graphPath !== 'string'
    || !isNullableString(value.selectionDigest)
    || !isNullableString(value.incidentId)
  ) {
    throw invalidTrace();
  }
  const metadata = parseMetadata(value.metadata);
  const spans = parseSpans(value.spans);
  return {
    bundleKind: 'run',
    runId: value.runId,
    compileId: value.compileId,
    graphPath: value.graphPath,
    selectionDigest: value.selectionDigest,
    incidentId: value.incidentId,
    metadata,
    spans,
  };
}

function parseMetadata(value: unknown): TraceBundleMetadataDto {
  if (
    !isRecord(value)
    || !hasExactKeys(value, METADATA_KEYS)
    || !Array.isArray(value.provenanceScopes)
    || typeof value.truncated !== 'boolean'
    || !isDecimal(value.droppedSpanCount)
    || !isDecimal(value.estimatedBytes)
  ) {
    throw invalidTrace();
  }
  return {
    provenanceScopes: value.provenanceScopes.map(parseProvenanceScope),
    truncated: value.truncated,
    droppedSpanCount: value.droppedSpanCount,
    estimatedBytes: value.estimatedBytes,
  };
}

function parseProvenanceScope(value: unknown): TraceProvenanceScopeDto {
  if (
    !isRecord(value)
    || !hasExactKeys(value, PROVENANCE_SCOPE_KEYS)
    || typeof value.projectSessionId !== 'string'
    || typeof value.graphPath !== 'string'
    || !isDecimal(value.graphRevision)
    || typeof value.registryFingerprint !== 'string'
    || !isStringRecord(value.resourceVersions)
    || !isDecimal(value.compileId)
  ) {
    throw invalidTrace();
  }
  return value as unknown as TraceProvenanceScopeDto;
}

function parseSpans(value: unknown): TraceSpanDto[] {
  if (!Array.isArray(value)) throw invalidTrace();
  return value.map(parseTraceSpan);
}

function parseTraceSpan(value: unknown): TraceSpanDto {
  if (!isRecord(value) || !hasExactKeys(value, SPAN_KEYS)) throw invalidTrace();
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
    || !isOutcome(value.outcome)
  ) {
    throw invalidTrace();
  }
  return {
    spanId: value.spanId,
    parentSpanId: value.parentSpanId,
    runId: value.runId,
    operationId: value.operationId,
    activationId: value.activationId,
    attemptId: value.attemptId,
    kind: value.kind as TraceKindDto,
    startedAt: value.startedAt,
    finishedAt: value.finishedAt,
    outcome: value.outcome,
    correlation,
  };
}

function parseCorrelation(value: unknown): TraceCorrelationDto {
  if (
    !isRecord(value)
    || !hasExactKeys(value, CORRELATION_KEYS)
    || typeof value.projectSessionId !== 'string'
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
  if (!isRecord(value) || !hasExactKeys(value, ['cleanup'])) return false;
  return isRecord(value.cleanup)
    && hasExactKeys(value.cleanup, ['errorCount', 'panicking'])
    && isDecimal(value.cleanup.errorCount)
    && typeof value.cleanup.panicking === 'boolean';
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function hasExactKeys(value: Record<string, unknown>, keys: readonly string[]): boolean {
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
  return new Error('Invalid trace bundle response');
}
