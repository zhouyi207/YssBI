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
}

export interface ImportDialogOptions {
    onSelect: (type: "csv" | "xlsx" | "sql" | "api") => void;
}