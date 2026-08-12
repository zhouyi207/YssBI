import type { Pin, Node as DomainNode } from '../domain';
import type { PinView } from '../store/graph';
import type {
    DiagnosticDto,
    NodeDisplayDto,
    ParameterEditorDto,
} from '../dto/editorProjection';

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
export interface UINode extends Omit<DomainNode, 'inputs' | 'outputs'> {
    /** Rust editor projection-authored layout style; view-only and not persisted by React. */
    uiStyle: string;
    position: { x: number; y: number };
    isInternal?: boolean;
    paramsKind?: 'none' | 'variable' | 'subGraph' | 'dataFrame';
    variableId?: string;
    variableName?: string;
    variableType?: string;
    subGraphPath?: string;
    dataframeId?: string;
    display?: NodeDisplayDto;
    parameterEditors?: ParameterEditorDto[];
    diagnostics?: DiagnosticDto[];
    centerSymbol?: string;
    inputs: PinView[];
    outputs: PinView[];
}

/**
 * 节点类（可变工具对象；画布渲染请使用 `UINode` + `toUiNode`）
 * 提供节点的克隆和操作方法
 */
export class Node implements UINode {
    id: string;
    nodeType: string;
    category: string[];
    title: string;
    inputs: PinView[];
    outputs: PinView[];
    uiStyle: string;
    description?: string;
    position: { x: number; y: number };
    isInternal: boolean;
    paramsKind?: 'none' | 'variable' | 'subGraph' | 'dataFrame';
    variableId?: string;
    variableName?: string;
    variableType?: string;
    subGraphPath?: string;
    dataframeId?: string;
    display?: NodeDisplayDto;
    parameterEditors?: ParameterEditorDto[];
    diagnostics?: DiagnosticDto[];
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
        this.paramsKind = data.paramsKind;
        this.variableId = data.variableId;
        this.variableName = data.variableName;
        this.variableType = data.variableType;
        this.subGraphPath = data.subGraphPath;
        this.dataframeId = data.dataframeId;
        this.display = data.display;
        this.parameterEditors = data.parameterEditors;
        this.diagnostics = data.diagnostics;
        this.centerSymbol = data.centerSymbol;
    }

    get noHeader(): boolean {
        return this.uiStyle === "math";
    }

    addInput(pin: Pin): void {
        this.inputs.push({
            ...pin,
            connected: false,
            linkCount: 0,
            connectionIds: [],
        });
    }

    clone(): Node {
        return new Node({
            id: this.id,
            nodeType: this.nodeType,
            category: [...this.category],
            title: this.title,
            inputs: this.inputs.map((p) => ({ ...p, connectionIds: [...p.connectionIds] })),
            outputs: this.outputs.map((p) => ({ ...p, connectionIds: [...p.connectionIds] })),
            uiStyle: this.uiStyle,
            description: this.description,
            position: { ...this.position },
            isInternal: this.isInternal,
            paramsKind: this.paramsKind,
            variableId: this.variableId,
            variableName: this.variableName,
            variableType: this.variableType,
            subGraphPath: this.subGraphPath,
            dataframeId: this.dataframeId,
            display: this.display,
            parameterEditors: this.parameterEditors,
            diagnostics: this.diagnostics,
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
        startX: number;
        startY: number;
        lastX: number;
        lastY: number;
        moved: boolean;
        groupId?: string;
    }
    | {
        type: "connect";
        startPin: Pin;
        startX: number;     // 屏幕坐标
        startY: number;
        currentX: number;   // 屏幕坐标（用于 hit-test）
        currentY: number;
        /** 连接线终点的世界坐标（用于多 editor 同步渲染） */
        worldX?: number;
        worldY?: number;
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
        /** 正在被拖拽的节点 ID 列表（用于多 editor 同步应用偏移） */
        dragNodeIds?: string[];
        /** 拖拽期间的累计偏移（世界坐标） */
        dragDelta?: { x: number; y: number };
    }
    | null;

export type ConnectGesture = Extract<NonNullable<EditorGesture>, { type: 'connect' }>;

/** Narrow `EditorGesture` to an active connect drag, or null. */
export function getConnectGesture(gesture: EditorGesture): ConnectGesture | null {
    return gesture?.type === 'connect' ? gesture : null;
}

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
