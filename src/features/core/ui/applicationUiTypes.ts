import type {
  DialogOptions,
  InputDialogOptions,
  MessageDialogOptions,
  ProgressState,
} from "@/shared/types/ui/types";

export type ImportDataSourceType =
  | "csv"
  | "xlsx"
  | "sqlite"
  | "postgres"
  | "mysql"
  | "mariadb"
  | "api";

export interface ImportDialogOptions {
  onSelect: (type: ImportDataSourceType) => void;
}

export interface SqliteTableSelectDialogOptions {
  dbPath: string;
  tables: string[];
  onSelect: (table: string) => void;
}

export interface ExcelSheetSelectDialogOptions {
  filePath: string;
  sheets: string[];
  onSelect: (sheet: string) => void;
}

export interface SqlConnectionDialogOptions {
  engine: "postgres" | "mysql" | "mariadb";
  onConnect: (connectionString: string) => void;
}

export interface SqlRemoteTableSelectDialogOptions {
  connectionString: string;
  engine: "postgres" | "mysql" | "mariadb";
  tables: string[];
  onSelect: (table: string) => void;
}

/** UI state exposed to the application composition root for rendering global overlays. */
export type ApplicationUiModal =
  | { readonly id: string; readonly type: "message"; readonly options: MessageDialogOptions }
  | { readonly id: string; readonly type: "confirm"; readonly options: DialogOptions }
  | { readonly id: string; readonly type: "input"; readonly options: InputDialogOptions }
  | { readonly id: string; readonly type: "import"; readonly options: ImportDialogOptions }
  | {
      readonly id: string;
      readonly type: "sqliteTableSelect";
      readonly options: SqliteTableSelectDialogOptions;
    }
  | {
      readonly id: string;
      readonly type: "excelSheetSelect";
      readonly options: ExcelSheetSelectDialogOptions;
    }
  | {
      readonly id: string;
      readonly type: "sqlConnection";
      readonly options: SqlConnectionDialogOptions;
    }
  | {
      readonly id: string;
      readonly type: "sqlRemoteTableSelect";
      readonly options: SqlRemoteTableSelectDialogOptions;
    };

export interface ApplicationUiState {
  readonly modals: readonly ApplicationUiModal[];
  readonly progress: ProgressState | null;
}
