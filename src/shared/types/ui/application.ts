import type {
  DialogOptions,
  ExcelSheetSelectDialogOptions,
  ImportDialogOptions,
  InputDialogOptions,
  MessageDialogOptions,
  ProgressState,
  SqlConnectionDialogOptions,
  SqliteTableSelectDialogOptions,
  SqlRemoteTableSelectDialogOptions,
} from './types';

/** UI state exposed to the application composition root for rendering global overlays. */
export type ApplicationUiModal =
  | { readonly id: string; readonly type: 'message'; readonly options: MessageDialogOptions }
  | { readonly id: string; readonly type: 'confirm'; readonly options: DialogOptions }
  | { readonly id: string; readonly type: 'input'; readonly options: InputDialogOptions }
  | { readonly id: string; readonly type: 'import'; readonly options: ImportDialogOptions }
  | {
      readonly id: string;
      readonly type: 'sqliteTableSelect';
      readonly options: SqliteTableSelectDialogOptions;
    }
  | {
      readonly id: string;
      readonly type: 'excelSheetSelect';
      readonly options: ExcelSheetSelectDialogOptions;
    }
  | {
      readonly id: string;
      readonly type: 'sqlConnection';
      readonly options: SqlConnectionDialogOptions;
    }
  | {
      readonly id: string;
      readonly type: 'sqlRemoteTableSelect';
      readonly options: SqlRemoteTableSelectDialogOptions;
    };

export interface ApplicationUiState {
  readonly modals: readonly ApplicationUiModal[];
  readonly progress: ProgressState | null;
}
