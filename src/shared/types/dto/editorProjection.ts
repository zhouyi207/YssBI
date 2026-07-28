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
}

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
