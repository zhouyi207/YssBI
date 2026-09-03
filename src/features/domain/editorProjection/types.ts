import type {
  CompilationOutcomeDto,
  DiagnosticDto,
  EditorConnectionProjectionDto,
  EditorGraphProjectionDto,
  EditorNodeProjectionDto,
  ProjectionBasisDto,
  EditorPortDto,
} from "@/shared/types/domain/editorProjection";

export type EditorProjectionNodeEntity = Omit<EditorNodeProjectionDto, "ports">;
export type EditorProjectionPortEntity = EditorPortDto;
export type EditorProjectionConnectionEntity = EditorConnectionProjectionDto;

export interface EditorProjectionEntities {
  basis: ProjectionBasisDto;
  graphPath: string;
  sourceRevision: number;
  nodes: Record<string, EditorProjectionNodeEntity>;
  ports: Record<string, EditorProjectionPortEntity>;
  connections: Record<string, EditorProjectionConnectionEntity>;
  portIdsByNodeId: Record<string, string[]>;
  connectionIdsByPortId: Record<string, string[]>;
  diagnostics: DiagnosticDto[];
  outcome: CompilationOutcomeDto;
  hasBlockingDiagnostics: boolean;
}

export type ValidatedEditorGraphProjection = EditorGraphProjectionDto;

export type * from "@/shared/types/domain/editorProjection";
