export interface WorksheetUiCapability {
  readonly updateDraft: (path: string, patch: Record<string, unknown>) => void;
  readonly setDirty: (path: string, dirty: boolean) => void;
}
