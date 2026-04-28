export type MessageType = "info" | "success" | "warning" | "error" | "log";

export interface Message {
    id: string;
    type: MessageType;
    content: string;
    duration?: number;
}

export interface DialogOptions {
    title: string;
    message: string;
    confirmText?: string;
    cancelText?: string;
    type?: "danger" | "info";
    onConfirm: () => void;
    onCancel?: () => void;
}

export interface InputDialogOptions {
    title: string;
    message?: string;
    label?: string;
    defaultValue?: string;
    placeholder?: string;
    confirmText?: string;
    cancelText?: string;
    onSubmit: (value: string) => void;
    onCancel?: () => void;
}

/** 导入数据源类型：文件类、SQL 数据库类、其他 */
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

/** PostgreSQL / MySQL / MariaDB 连接配置弹窗 */
export interface SqlConnectionDialogOptions {
    engine: "postgres" | "mysql" | "mariadb";
    onConnect: (connectionString: string) => void;
}

/** PostgreSQL / MySQL / MariaDB 选表弹窗（复用表选择 UI） */
export interface SqlRemoteTableSelectDialogOptions {
    connectionString: string;
    engine: "postgres" | "mysql" | "mariadb";
    tables: string[];
    onSelect: (table: string) => void;
}