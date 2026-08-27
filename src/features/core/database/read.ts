export interface DatabaseReadSnapshot {
  readonly id: string;
  readonly name: string;
  readonly revision: number;
  readonly columns: readonly { readonly name: string; readonly type: string }[];
}

export interface DatabaseReadCapability {
  readonly getDatabase: (id: string) => DatabaseReadSnapshot | null;
  readonly listDatabases: () => readonly DatabaseReadSnapshot[];
}
