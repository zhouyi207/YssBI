/**
 * 窗口几何状态相关类型。后端 (`src-tauri/src/window_state`) 是权威来源；
 * 前端通过 `WindowStateService` 读写，无 localStorage 持久化。
 */

/** 受持久化管理的窗口种类。 */
export type WindowKind =
  | "main"
  | "databaseEditor"
  | "sourceInspector"
  | "logs"
  | "plot"
  | "info"
  | "bayes";

/** 单个窗口的几何状态。 */
export interface WindowState {
  width: number;
  height: number;
  /** 物理像素坐标，`null` 表示尚未保存过位置 */
  x: number | null;
  y: number | null;
  isMaximized: boolean;
}
