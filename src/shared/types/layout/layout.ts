/**
 * UI Types - Layout
 *
 * 布局系统的 UI 状态类型
 * 用于管理编辑器的窗口布局
 */

/**
 * 布局方向
 */
export type LayoutDirection = 'row' | 'col';

/**
 * 布局节点类型
 */
export type LayoutNodeType = 'row' | 'col' | 'component';

/** 编辑器 Tab 语义类型（与 ResourceKind / DetailFocus 对齐） */
export type LayoutTabType = 'event' | 'function' | 'worksheet' | 'project' | 'setting';

/** Tab 挂载的编辑器组件名 */
export type LayoutTabComponent = 'GraphEditor' | 'WorksheetEditor';

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
}

/** 编辑器组节点 `data.params`（非 Tab 级字段） */
export interface EditorGroupNodeParams {
  selectedNodeIds?: string[];
}

/** Stable editor group identity for shared session context. */
export interface EditorGroupSnapshot {
  id: string;
}

/**
 * 布局节点
 * 表示布局树中的一个节点
 */
export interface LayoutNode {
  id: string;
  type: LayoutNodeType;
  parentId: string | null;
  children?: string[];

  // 布局属性
  size?: number;
  pixelSize?: number;
  minSize?: number;
  maxSize?: number;

  // 内容信息（仅用于 'component' 类型）
  data?: {
    component?: string;
    title?: string;
    isFixed?: boolean;
    params?: EditorGroupNodeParams;
    visible?: boolean;
    currentTab?: string | null;
    /** User explicitly hid detail panel; auto-show should not override. */
    userHidden?: boolean;
    /** Panel maximized via sash double-click. */
    maximized?: boolean;
    restoredPixelSize?: number;
    /** Editor group hidden while another group is maximized in editor_area. */
    groupMaximizedHidden?: boolean;
    /** editor_area: currently maximized editor group id. */
    maximizedGroupId?: string;
    /** editor_area: pixel sizes snapshot before group maximize. */
    restoredGridSizes?: Record<string, number>;
    /** Bottom panel tab views (Logs / Output / …). */
    panelViews?: { id: string; component: string }[];
    activePanelView?: string;
  };
}

/**
 * 布局树
 * 表示整个布局结构
 */
export type LayoutTree = Record<string, LayoutNode>;
