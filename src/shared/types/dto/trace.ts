export type TraceDecimalString = `${bigint}`;

export type TraceKindDto =
  | 'snapshot'
  | 'analysis'
  | 'lowering'
  | 'run'
  | 'operation'
  | 'relationalBackend'
  | 'resourceAcquire'
  | 'cleanup';

export type TraceStatusDto =
  | 'started'
  | 'succeeded'
  | 'failed'
  | 'cancelled'
  | 'blocked';

export type TraceValueDto =
  | { type: 'integer'; value: number }
  | { type: 'text'; value: string }
  | { type: 'redacted' };

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

export interface TraceRecordDto {
  sequence: TraceDecimalString;
  kind: TraceKindDto;
  status: TraceStatusDto;
  correlation: TraceCorrelationDto;
  fields: Record<string, TraceValueDto>;
}
