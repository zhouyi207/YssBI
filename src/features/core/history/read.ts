export interface HistoryReadSnapshot {
  readonly canUndo: boolean;
  readonly canRedo: boolean;
  readonly pending: boolean;
}

export interface HistoryReadCapability {
  readonly getHistory: () => HistoryReadSnapshot;
}
