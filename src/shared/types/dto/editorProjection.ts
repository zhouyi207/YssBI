import type { FunctionSignaturePin } from '@/shared/types/domain/graph';
import type { DataType } from '@/shared/types/domain/dataType';

export interface FunctionEditorProjectionDto {
  functionRevision: number;
  inputs: FunctionSignaturePin[];
  outputs: FunctionSignaturePin[];
}

export interface ProjectionBasisDto {
  graphPath: string;
  graphRevision: number;
  registryFingerprint: string;
  resourceVersions: Record<string, string>;
}

export type CompilationStageDto = 'analysis' | 'lowering';

export type CompilationOutcomeDto =
  | { type: 'success' }
  | { type: 'analysisBlocked' }
  | {
      type: 'internalFailure';
      stage: CompilationStageDto;
      code: string;
      nodeId: string | null;
    };

export interface EditorGraphProjectionDto {
  basis: ProjectionBasisDto;
  graphPath: string;
  sourceRevision: number;
  nodes: EditorNodeProjectionDto[];
  connections: EditorConnectionProjectionDto[];
  diagnostics: DiagnosticDto[];
  outcome: CompilationOutcomeDto;
  hasBlockingDiagnostics: boolean;
}

export interface EventGraphProjectionReplacementDto {
  graphPath: string;
  projection: EditorGraphProjectionDto;
  functionEditorProjection?: never;
}

export interface FunctionGraphProjectionReplacementDto {
  graphPath: string;
  projection: EditorGraphProjectionDto;
  functionEditorProjection: FunctionEditorProjectionDto;
}

export type GraphProjectionReplacementDto =
  | EventGraphProjectionReplacementDto
  | FunctionGraphProjectionReplacementDto;

export interface EditorNodeProjectionDto {
  graphPath: string;
  sourceRevision: number;
  nodeId: string;
  nodeTypeId: string;
  position: NodePositionDto;
  display: NodeDisplayDto;
  ports: ResolvedPortDto[];
  parameterEditors: ParameterEditorDto[];
  capabilities: NodeCapabilitiesDto;
  diagnostics: DiagnosticDto[];
}

export interface NodePositionDto {
  x: number;
  y: number;
}

export interface EditorConnectionProjectionDto {
  connectionId: string;
  output: PortAddressDto;
  input: PortAddressDto;
  order: string | null;
}

export interface NodeDisplayDto {
  title: string;
  description: string | null;
  userLabel: string | null;
  iconId: string | null;
  styleId: string | null;
}

export interface NodeCapabilitiesDto {
  managed: boolean;
  canCopy: boolean;
  canDelete: boolean;
  canEditLabel: boolean;
  canEditParameters: boolean;
  hasDynamicPorts: boolean;
  supportsInlineLiterals: boolean;
}

export interface ResolvedPortDto {
  address: PortAddressDto;
  templateKey: string;
  display: PortDisplayDto;
  direction: PortDirectionDto;
  kind: PortKindDto;
  instanceKind: PortInstanceKindDto;
  orphan: boolean;
  canRemove: boolean;
  connections: PortConnectionCapabilityDto;
  input: EditorInputBindingDto | null;
  resolvedType: TypeSummaryDto | null;
  resolvedSchema: SchemaSummaryDto | null;
  status: ResolvedPortStatusDto;
}

export type PortAddressDto =
  | { kind: 'declared'; nodeId: string; portKey: string }
  | { kind: 'instance'; nodeId: string; templateKey: string; instanceId: string };

export interface PortDisplayDto {
  label: string;
  instanceLabel: string | null;
}

export type PortDirectionDto = 'input' | 'output';
export type PortKindDto = 'data' | 'control' | 'effect';
export type PortInstanceKindDto = 'declared' | 'userCreated' | 'derived';

export interface PortConnectionCapabilityDto {
  current: number;
  maximum: number | null;
  ordered: boolean;
  canConnect: boolean;
}

export interface EditorInputBindingDto {
  literalOverride: unknown | null;
  protocolDefault: unknown | null;
  effective: EffectiveInputBindingKindDto;
}

export type EffectiveInputBindingKindDto =
  | 'connections'
  | 'literal'
  | 'protocolDefault'
  | 'unbound';

export interface TypeSummaryDto {
  display: string;
  resolved: boolean;
  dataType: DataType | null;
}

export interface SchemaSummaryDto {
  kind: SchemaSummaryKindDto;
  fields: SchemaFieldDto[];
}

export interface SchemaFieldDto {
  name: string;
  scalarType: RelationalScalarTypeDto;
}

export type RelationalScalarTypeDto =
  | 'boolean'
  | 'int64'
  | 'float64'
  | 'string'
  | 'date'
  | 'dateTime'
  | 'unknown';

export type SchemaSummaryKindDto =
  | 'input'
  | 'project'
  | 'append'
  | 'rename'
  | 'filter'
  | 'derived';

export type ResolvedPortStatusDto = 'resolved' | 'orphan';

export interface ParameterEditorDto {
  key: string;
  display: ParameterDisplayDto;
  editor: ParameterEditorKindDto;
  multiline: boolean;
  value: unknown | null;
  configuration: SchemaAwareParameterEditorDto | null;
}

export interface DataframeColumnOptionDto {
  name: string;
  dataType: RelationalScalarTypeDto;
}

export type FilterOperatorDto =
  | 'equal'
  | 'notEqual'
  | 'lessThan'
  | 'lessThanOrEqual'
  | 'greaterThan'
  | 'greaterThanOrEqual'
  | 'isNull'
  | 'isNotNull';

export type FilterLiteralDto =
  | { type: 'boolean'; value: boolean }
  | { type: 'integer'; value: string }
  | { type: 'decimal'; value: string }
  | { type: 'string'; value: string };

export interface FilterPredicateDto {
  column: string;
  operator: FilterOperatorDto;
  value?: FilterLiteralDto;
}

export type SchemaAwareParameterEditorDto =
  | {
      kind: 'projectColumns';
      available: boolean;
      unavailableReason: string | null;
      options: DataframeColumnOptionDto[];
      value: string[];
    }
  | {
      kind: 'filterPredicate';
      available: boolean;
      unavailableReason: string | null;
      columns: Array<DataframeColumnOptionDto & {
        operators: FilterOperatorDto[];
        literalTypes: FilterLiteralDto['type'][];
      }>;
      value: FilterPredicateDto | null;
    };



export interface ParameterDisplayDto {
  title: string;
  description: string | null;
}

export type ParameterEditorKindDto =
  | 'auto'
  | 'text'
  | 'number'
  | 'toggle'
  | 'select'
  | 'resource';

export interface DiagnosticDto {
  code: string;
  message: string;
  severity: DiagnosticSeverityDto;
  blocking: boolean;
  location: DiagnosticLocationDto;
  related: DiagnosticLocationDto[];
}

export type DiagnosticSeverityDto = 'error' | 'warning' | 'information';

export type DiagnosticLocationDto =
  | { kind: 'graph' }
  | { kind: 'node'; nodeId: string }
  | { kind: 'port'; address: PortAddressDto }
  | { kind: 'connection'; connectionId: string }
  | { kind: 'parameter'; nodeId: string; key: string }
  | { kind: 'resource'; identity: string };
