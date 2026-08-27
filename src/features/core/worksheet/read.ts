export interface WorksheetReadSnapshot {
  readonly path: string;
  readonly revision: number;
  readonly databaseId: string;
  readonly chartType: string;
}

export interface WorksheetReadCapability {
  readonly getWorksheet: (path: string) => WorksheetReadSnapshot | null;
  readonly listWorksheets: () => readonly WorksheetReadSnapshot[];
}
