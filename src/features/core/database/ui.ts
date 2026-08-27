export interface DatabaseUiCapability {
  readonly selectDatabase: (id: string | null) => void;
  readonly setQuery: (id: string, query: string) => void;
}
