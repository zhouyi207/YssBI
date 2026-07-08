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
 * 表示布局组件中的一个标签页
 */
export interface LayoutTab {
  id: string;
  title: string;
  component: LayoutTabComponent;
  type: LayoutTabType;
  isDirty?: boolean;
}

/** 编辑器组节点 `data.params`（非 Tab 级字段） */
export interface EditorGroupNodeParams {
  selectedNodeIds?: string[];
}

/** `useEditorGroups` / split 等共用的编辑器组快照 */
export interface EditorGroupSnapshot {
  id: string;
  tabs: LayoutTab[];
  activeTabId: string | null;
  selectedNodeIds: string[];
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
    tabs?: LayoutTab[];
    activeTabId?: string;
    currentTab?: string | null;
  };
}

/**
 * 布局树
 * 表示整个布局结构
 */
export type LayoutTree = Record<string, LayoutNode>;
