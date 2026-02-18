import type { Pin, Node as DomainNode } from '../domain';

/**
 * UI Types - Editor
 * 
 * 编辑器 UI 状态类型
 * 这些类型只在前端使用，不会发送到后端
 */

/**
 * UI 节点
 * 扩展领域节点，添加 UI 特定的属性和方法
 */
export interface UINode extends DomainNode {
    position: { x: number; y: number };
    isInternal?: boolean;
    variableId?: string;
    variableName?: string;
    variableType?: string;
    subGraphId?: string;
    dataframeId?: string;
    columnName?: string;
    columnType?: string;
    centerSymbol?: string;
}

/**
 * 节点类
 * 提供节点的克隆和操作方法
 */
export class Node implements UINode {
    id: string;
    nodeType: string;
    category: string[];
    title: string;
    inputs: Pin[];
    outputs: Pin[];
    uiStyle: string;
    description?: string;
    position: { x: number; y: number };
    isInternal: boolean;
    variableId?: string;
    variableName?: string;
    variableType?: string;
    subGraphId?: string;
    dataframeId?: string;
    columnName?: string;
    columnType?: string;
    centerSymbol?: string;

    constructor(data: UINode) {
        this.id = data.id;
        this.nodeType = data.nodeType;
        this.category = data.category;
        this.title = data.title;
        this.inputs = data.inputs;
        this.outputs = data.outputs;
        this.uiStyle = data.uiStyle;
        this.description = data.description;
        this.position = data.position;
        this.isInternal = data.isInternal || false;
        this.variableId = data.variableId;
        this.variableName = data.variableName;
        this.variableType = data.variableType;
        this.subGraphId = data.subGraphId;
        this.dataframeId = data.dataframeId;
        this.columnName = data.columnName;
        this.columnType = data.columnType;
        this.centerSymbol = data.centerSymbol;
    }

    get noHeader(): boolean {
        return this.uiStyle === "math";
    }

    addInput(pin: Pin): void {
        this.inputs.push(pin);
    }

    clone(): Node {
        return new Node({
            id: this.id,
            nodeType: this.nodeType,
            category: [...this.category],
            title: this.title,
            inputs: this.inputs.map(p => ({ ...p, links: p.links ? [...p.links] : [] })),
            outputs: this.outputs.map(p => ({ ...p, links: p.links ? [...p.links] : [] })),
            uiStyle: this.uiStyle,
            description: this.description,
            position: { ...this.position },
            isInternal: this.isInternal,
            variableId: this.variableId,
            variableName: this.variableName,
            variableType: this.variableType,
            subGraphId: this.subGraphId,
            dataframeId: this.dataframeId,
            columnName: this.columnName,
            columnType: this.columnType,
            centerSymbol: this.centerSymbol,
        });
    }
}

/**
 * 编辑器手势类型
 * 表示用户当前的交互状态
 */
export type EditorGesture =
    | {
        type: "pan";
        lastX: number;
        lastY: number;
        moved: boolean;
        groupId?: string;
    }
    | {
        type: "select";
        startX: number;
        startY: number;
        currentX: number;
        currentY: number;
        groupId?: string;
    }
    | {
        type: "connect";
        startPin: Pin;
        startX: number;     // 屏幕坐标
        startY: number;
        currentX: number;
        currentY: number;
        isReconnect?: boolean;
        groupId?: string;
    }
    | {
        type: "drag";
        nodeId?: string;
        lastX: number;
        lastY: number;
        moved: boolean;
        groupId?: string;
        /** 拖拽期间的视觉偏移（世界坐标），仅在拖拽时更新，不写回 store，减少卡顿 */
        dragDelta?: { x: number; y: number };
    }
    | null;

/**
 * 编辑器标签页
 * 表示编辑器中的一个标签页
 */
export interface EditorTab {
    id: string;
    title: string;
    type: "event" | "function" | "macro" | "project" | "setting";
    isDirty?: boolean;
}

/**
 * 编辑器组
 * 表示一个编辑器分组（可以包含多个标签页）
 */
export interface EditorGroup {
    id: string;
    tabs: EditorTab[];
    activeTabId: string | null;
    selectedNodeIds: string[];
    width?: number;  // 用于调整大小
}

/**
 * 拖拽状态
 * 表示从节点模板拖拽到画布的状态
 */
export type DragState = {
    type: "node-template";
    template: any;
    x: number;
    y: number;
    startX: number;
    startY: number;
} | null;

/**
 * 上下文菜单状态
 */
export interface ContextMenuState {
    x: number;
    y: number;
    items: ContextMenuItem[];
}

/**
 * 上下文菜单项
 */
export interface ContextMenuItem {
    label: string;
    action: () => void;
    disabled?: boolean;
    divider?: boolean;
}

/**
 * 待处理的连接
 * 表示正在创建但尚未完成的连接
 */
export interface PendingConnection {
    fromPinId: string;
    fromNodeId: string;
    startX: number;
    startY: number;
}
