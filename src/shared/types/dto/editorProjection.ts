export interface ProjectionBasisDto {
  graphPath: string;
  graphRevision: number;
  registryFingerprint: number[];
  resourceVersions: Record<string, string>;
}

export interface EditorGraphProjectionDto {
  basis: ProjectionBasisDto;
  graphPath: string;
  sourceRevision: number;
  nodes: EditorNodeProjectionDto[];
  connections: EditorConnectionProjectionDto[];
  diagnostics: DiagnosticDto[];
  hasBlockingDiagnostics: boolean;
}

export interface GraphProjectionReplacementDto {
  graphPath: string;
  projection: EditorGraphProjectionDto;
}

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

const relationalScalarTypes = new Set<RelationalScalarTypeDto>([
  'boolean', 'int64', 'float64', 'string', 'date', 'dateTime', 'unknown',
]);
const filterOperators = new Set<FilterOperatorDto>([
  'equal', 'notEqual', 'lessThan', 'lessThanOrEqual', 'greaterThan',
  'greaterThanOrEqual', 'isNull', 'isNotNull',
]);

function hasExactKeys(value: unknown, keys: readonly string[]): value is Record<string, unknown> {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return false;
  const actual = Object.keys(value);
  return actual.length === keys.length && keys.every((key) => actual.includes(key));
}

function isColumnOption(value: unknown, withOperators: boolean): boolean {
  const keys = withOperators
    ? ['name', 'dataType', 'operators', 'literalTypes']
    : ['name', 'dataType'];
  if (!hasExactKeys(value, keys)) return false;
  return typeof value.name === 'string'
    && relationalScalarTypes.has(value.dataType as RelationalScalarTypeDto)
    && (!withOperators || (Array.isArray(value.operators)
      && value.operators.every((operator) => filterOperators.has(operator as FilterOperatorDto))
      && Array.isArray(value.literalTypes)
      && value.literalTypes.every((type) => (
        type === 'boolean' || type === 'integer' || type === 'decimal' || type === 'string'
      ))));
}

function isFilterLiteral(value: unknown): value is FilterLiteralDto {
  if (!hasExactKeys(value, ['type', 'value'])) return false;
  if (value.type === 'boolean') return typeof value.value === 'boolean';
  return (value.type === 'integer' || value.type === 'decimal' || value.type === 'string')
    && typeof value.value === 'string';
}

function isFilterPredicate(value: unknown): value is FilterPredicateDto {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return false;
  const candidate = value as Record<string, unknown>;
  const nullCheck = candidate.operator === 'isNull' || candidate.operator === 'isNotNull';
  if (!hasExactKeys(candidate, nullCheck ? ['column', 'operator'] : ['column', 'operator', 'value'])) {
    return false;
  }
  return typeof candidate.column === 'string'
    && filterOperators.has(candidate.operator as FilterOperatorDto)
    && (nullCheck || isFilterLiteral(candidate.value));
}

export function isSchemaAwareParameterEditorDto(
  value: unknown,
): value is SchemaAwareParameterEditorDto {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return false;
  const candidate = value as Record<string, unknown>;
  const commonValid = typeof candidate.available === 'boolean'
    && (candidate.unavailableReason === null || typeof candidate.unavailableReason === 'string');
  if (candidate.kind === 'projectColumns') {
    return hasExactKeys(candidate, [
      'kind', 'available', 'unavailableReason', 'options', 'value',
    ]) && commonValid
      && Array.isArray(candidate.options) && candidate.options.every((option) => isColumnOption(option, false))
      && Array.isArray(candidate.value) && candidate.value.every((column) => typeof column === 'string');
  }
  if (candidate.kind === 'filterPredicate') {
    return hasExactKeys(candidate, [
      'kind', 'available', 'unavailableReason', 'columns', 'value',
    ]) && commonValid
      && Array.isArray(candidate.columns) && candidate.columns.every((column) => isColumnOption(column, true))
      && (candidate.value === null || isFilterPredicate(candidate.value));
  }
  return false;
}

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
