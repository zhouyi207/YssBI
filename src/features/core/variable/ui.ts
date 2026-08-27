export interface VariableUiCapability {
  readonly setDraftValue: (id: string, value: unknown) => void;
  readonly setScope: (scope: string) => void;
}
