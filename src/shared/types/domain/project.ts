import type { Variable } from "./variable";
import type { DatabaseEngineDTO } from "./database";
import type { FunctionEditorProjectionDto } from "./editorProjection";
import type { FunctionSignatureDto, ResourceKeyDto } from "./editorMutation";
import type { DataType } from "./dataType";
import type { DataValue } from "./dataValue";
import type { ChartType } from "./chart";

export interface ProjectRecordRow {
  id: string;
  name: string;
  path: string;
  createdAt: string;
  lastOpenedAt: string | null;
  isFavorite: boolean;
  rootIdentity: string;
}

export type LifecycleMutationKind =
  | "saveAs"
  | "create"
  | "delete"
  | "registryCleanup"
  | "load"
  | "clear";
export type LifecycleMutationPhase =
  | "destinationCommitted"
  | "registryCommitted"
  | "authorityCommitted";
export type LifecycleMutationOutcome =
  | "committed"
  | "registryFailed"
  | "activationFailed"
  | "registryPending";

export interface LifecycleRecoveryDto {
  required: boolean;
  action: string;
  path: string | null;
  identity: string | null;
}

export interface LifecycleMutationResultDto {
  operationId: string;
  kind: LifecycleMutationKind;
  oldProjectInstanceId: string | null;
  newProjectInstanceId: string | null;
  phase: LifecycleMutationPhase;
  outcome: LifecycleMutationOutcome;
  record: ProjectRecordRow | null;
  path: string | null;
  recovery: LifecycleRecoveryDto | null;
  invalidation: { project: boolean; registry: boolean };
}

export interface ScanProjectsResult {
  discovered: number;
  newlyRegistered: number;
  projects: ProjectRecordRow[];
}

export interface CleanupInvalidProjectsResult {
  removed: number;
}

interface ProjectGraphIndexRowBase {
  path: string;
  name: string;
  revision: number;
}

export interface ProjectEventGraphIndexRow extends ProjectGraphIndexRowBase {
  type: "event";
}

export interface ProjectFunctionGraphIndexRow extends ProjectGraphIndexRowBase {
  type: "function";
  functionRevision: number;
  functionSignature: FunctionSignatureDto;
  functionEditorProjection: FunctionEditorProjectionDto;
}

export type ProjectGraphIndexRow = ProjectEventGraphIndexRow | ProjectFunctionGraphIndexRow;

export interface ProjectChartIndexRow {
  chartPath: string;
  name: string;
  databaseId: string;
  chartType: ChartType;
  revision: number;
}

export interface ProjectVariableIndexRow {
  id: string;
  resourcePath: string;
  revision: number;
  name: string;
  dataType: DataType;
  dataValue: DataValue;
  description: string;
  scope: Variable["scope"];
  tags: string[];
  ownerGraphPath?: string | null;
  ownerGraphName?: string | null;
  ownerGraphKind?: "event" | "function" | null;
}

export interface ProjectDatabaseIndexRow {
  id: string;
  resourcePath: string;
  revision: number;
  engine: DatabaseEngineDTO;
  schemaVersion: number;
  required: boolean;
  name: string | null;
}

export interface ProjectIndexRow {
  projectInstanceId: string;
  projectName: string;
  exportTime: string;
  publicationRevision: number;
  graphs: ProjectGraphIndexRow[];
  charts: ProjectChartIndexRow[];
  variables: ProjectVariableIndexRow[];
  databases: ProjectDatabaseIndexRow[];
}

export interface ProjectSaveResultDto {
  projectInstanceId: string;
  operationId: string;
  publicationRevision: number;
  affectedResources: ResourceKeyDto[];
  indexInvalidated: boolean;
}
