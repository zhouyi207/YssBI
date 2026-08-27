export interface GraphReadSnapshot {
  readonly graphPath: string;
  readonly revision: number;
  readonly nodes: readonly unknown[];
  readonly connections: readonly unknown[];
}

export interface GraphReadCapability {
  readonly getGraph: (graphPath: string) => GraphReadSnapshot | null;
  readonly listGraphs: () => readonly GraphReadSnapshot[];
}
