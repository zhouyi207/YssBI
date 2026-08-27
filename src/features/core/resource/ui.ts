export interface ResourceUiCapability {
  readonly markDirty: (resourcePath: string, dirty: boolean) => void;
  readonly setDraft: (resourcePath: string, draft: unknown) => void;
}
