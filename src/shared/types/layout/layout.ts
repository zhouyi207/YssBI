/**
 * UI Types - Layout
 *
 * 布局系统的 UI 状态类型
 * 用于管理编辑器的窗口布局
 */

/** 编辑器 Tab 语义类型（与 ResourceKind / DetailFocus 对齐） */
export type LayoutTabType = "event" | "function" | "worksheet";

/** Tab 挂载的编辑器组件名 */
export type LayoutTabComponent = "GraphEditor" | "WorksheetEditor";

/**
 * 布局标签页
 *
 * - **Graph tab**（`type: 'event' | 'function'`）：`id` 即图资源相对路径（`Graph.path`），
 *   与 `ResourceRef.id`、`GraphData.path`、`graphPath` API 参数同值；禁止 tab 级 UUID。
 * - **Worksheet tab**：`id` 为 worksheet 资源 id（非图 path）。
 */
export interface LayoutTab {
  /** Graph tab: project-relative graph path; worksheet tab: worksheet resource id */
  id: string;
  component: LayoutTabComponent;
  type: LayoutTabType;
  /**
   * `false` = preview tab (italic, one per editor group, replaceable until pinned).
   * `true` or omitted = pinned / permanent.
   */
  pinned?: boolean;
  /** VS Code sticky tab — stays at the leading edge of the tab strip. */
  sticky?: boolean;
}

/** Stable editor group identity for shared session context. */
export interface EditorGroupSnapshot {
  id: string;
}
