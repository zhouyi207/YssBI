export interface MessageDialogOptions {
  title: string;
  message: string;
  closeText: string;
  type: "info" | "warning" | "error";
  incidentId?: string | null;
  incidentLabel?: string;
}

export interface DialogOptions {
  title: string;
  message: string;
  confirmText?: string;
  cancelText?: string;
  /** Optional third action between confirm and cancel; commonly "Don't save" / "Discard". */
  discardText?: string;
  type?: "danger" | "info";
  onConfirm: () => void;
  onCancel?: () => void;
  /** Triggered by the discard action when `discardText` is provided. */
  onDiscard?: () => void;
}

/** Result of a tri-state confirm dialog (Save / Discard / Cancel). */
export type ConfirmTriResult = "confirm" | "discard" | "cancel";

/**
 * 全局进度蒙层状态。
 * - `stage`：主标题，描述当前阶段。
 * - `detail`：可选的次要文案（如具体子任务）。
 * - `percent`：0~1，未提供时显示不确定（indeterminate）进度条。
 */
export interface ProgressState {
  stage: string;
  detail?: string;
  percent?: number;
  /** 为 true 时在蒙层右上角显示关闭/取消按钮。 */
  cancelable?: boolean;
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
