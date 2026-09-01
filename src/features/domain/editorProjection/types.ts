import type {
  CompilationOutcomeDto,
  DiagnosticDto,
  EditorConnectionProjectionDto,
  EditorGraphProjectionDto,
  EditorNodeProjectionDto,
  ProjectionBasisDto,
  ResolvedPortDto,
} from "@/shared/types/domain/editorProjection";

export type EditorProjectionNodeEntity = Omit<EditorNodeProjectionDto, "ports">;
export type EditorProjectionPortEntity = ResolvedPortDto;
export type EditorProjectionConnectionEntity = EditorConnectionProjectionDto;

export interface EditorProjectionEntities {
  basis: ProjectionBasisDto;
  graphPath: string;
  sourceRevision: number;
  nodes: Record<string, EditorProjectionNodeEntity>;
  ports: Record<string, EditorProjectionPortEntity>;
  connections: Record<string, EditorProjectionConnectionEntity>;
  portKeysByNodeId: Record<string, string[]>;
  connectionIdsByPortKey: Record<string, string[]>;
  diagnostics: DiagnosticDto[];
  outcome: CompilationOutcomeDto;
  hasBlockingDiagnostics: boolean;
}

export type ValidatedEditorGraphProjection = EditorGraphProjectionDto;

export type * from "@/shared/types/domain/editorProjection";
