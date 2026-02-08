
export type LayoutDirection = 'row' | 'col';

export type LayoutNodeType = 'row' | 'col' | 'component';

export interface LayoutTab {
    id: string;
    title: string;
    component: string;
    type?: "event" | "function" | "macro" | "project" | "setting";
    params?: Record<string, any>;
}

export interface LayoutNode {
    id: string;
    type: LayoutNodeType;
    parentId: string | null;
    children?: string[];

    // Layout properties
    size?: number; // Flex weight (e.g. 1) or Fixed size in pixels
    pixelSize?: number; // Explicit pixel size (priority over flex?) - simplified to just using one or logic to distinguishing
    minSize?: number;
    maxSize?: number;

    // Content info (only for 'component' type)
    data?: {
        component?: string;
        title?: string;
        isFixed?: boolean;
        params?: Record<string, any>;
        visible?: boolean;
        tabs?: LayoutTab[];
        activeTabId?: string; // 好像没用啊
        currentTab?: string | null;
    };
}

export type LayoutTree = Record<string, LayoutNode>;
