export interface ExecutionReadSnapshot {
  readonly graphPath: string;
  readonly status: string;
  readonly runId: string | null;
  readonly output: readonly unknown[];
}

export interface ExecutionReadCapability {
  readonly getExecution: (graphPath: string) => ExecutionReadSnapshot | null;
}
