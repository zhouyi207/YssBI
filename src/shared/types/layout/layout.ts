/**
 * 布局方向
 */
export type LayoutDirection = 'row' | 'col';

/**
 * 布局节点类型
 */
export type LayoutNodeType = 'row' | 'col' | 'component';

/**
 * 布局标签页
 * 表示布局组件中的一个标签页
 */
export interface LayoutTab {
    id: string;
    title: string;
    component: string;
    type?: "event" | "function" | "macro" | "project" | "setting";
    params?: Record<string, any>;
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
    size?: number;          // Flex 权重（如 1）或固定像素大小
    pixelSize?: number;     // 显式像素大小（优先于 flex）
    minSize?: number;       // 最小尺寸
    maxSize?: number;       // 最大尺寸

    // 内容信息（仅用于 'component' 类型）
    data?: {
        component?: string;
        title?: string;
        isFixed?: boolean;
        params?: Record<string, any>;
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
