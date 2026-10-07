// Generated from yss-plugin-protocol. Do not edit.
export type Contributions = {
  commands: Array<PluginCommand>;
  taskTypes: Array<TaskType>;
  views: Array<PluginView>;
};
export type ExecutionMode = "trustedNative" | "sandboxRequired";
export type InstalledPlugin = {
  enabled: boolean;
  grantedBudget?: ResourceBudget;
  installationGeneration: string;
  manifest: PluginManifest;
  packageDigest: string;
  processState: string;
  signerKey?: string;
};
export type PackageInspection = {
  manifest: PluginManifest;
  packageDigest: string;
  previousSignerKey?: string | null;
  signerKey: string;
};
export type PluginCommand = { id: string; title: string };
export type PluginDiagnostic = {
  instanceId: string;
  pluginId: string;
  stderr: string;
  taskIds: Array<string>;
  truncated: boolean;
};
export type PluginFailure = { code: string; details?: unknown; incidentId?: string | null };
export type PluginManifest = {
  cacheDirectories?: Array<string>;
  contributes: Contributions;
  description: string;
  executable: string;
  execution: ExecutionMode;
  hostApi: string;
  id: string;
  name: string;
  permissions: Array<string>;
  protocol: ProtocolRange;
  publisher: string;
  resourceBudget: ResourceBudget;
  schemaVersion: number;
  target: string;
  uiMethods: Array<string>;
  version: string;
};
export type PluginStorageUsage = {
  budgetBytes: number;
  cacheBytes: number;
  enforcement: string;
  pluginId: string;
  usedBytes: number;
};
export type PluginView = {
  entry: string;
  id: string;
  location: ViewLocation;
  scope: ViewScope;
  title: string;
};
export type ProtocolRange = {
  major: number;
  maxMinor: number;
  minMinor: number;
  requiredFeatures: Array<string>;
};
export type ResourceBudget = {
  activeTasks: number;
  frameBytes: number;
  pendingRequests: number;
  privateStorageBytes: number;
  queuedBytes: number;
  snapshotBytes: number;
  views: number;
};
export type TaskHistoryPage = { nextCursor?: string | null; tasks: Array<TaskSnapshot> };
export type TaskSnapshot = {
  error?: PluginFailure | null;
  operationId: string;
  packageDigest: string;
  pluginId: string;
  progress?: unknown;
  result?: unknown;
  revision: string;
  state: TaskState;
  taskId: string;
};
export type TaskState =
  | "admitted"
  | "running"
  | "cancelRequested"
  | "succeeded"
  | "failed"
  | "cancelled"
  | "outcomeUnknown";
export type TaskType = { id: string; producesArtifacts: boolean };
export type ViewLocation = "sidebar" | "editor";
export type ViewScope = "application" | "project";
export type ViewSession = { html: string; installationGeneration: string; sessionId: string };
