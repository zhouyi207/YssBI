import type { Pin, Node as DomainNode } from '../domain';
import type { PinView } from '../store/graph';

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
            centerSymbol: this.centerSymbol,
        });
    }
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
