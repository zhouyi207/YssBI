export interface SettingsUiCapability {
  readonly setTheme: (theme: string) => void;
  readonly setEditorOption: (key: string, value: string | number | boolean) => void;
}
