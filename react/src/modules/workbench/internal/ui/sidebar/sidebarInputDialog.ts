export interface SidebarInputDialogState {
  readonly title: string;
  readonly value: string;
  readonly submitLabel?: string;
  readonly error?: string | null;
  readonly onSubmit: (value: string) => void | Promise<void>;
}
