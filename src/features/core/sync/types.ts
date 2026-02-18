// src/features/core/sync/types.ts

import type { ProjectData, Graph, Variable, VariableScope, Node } from '@/shared/types/domain';
import type { NodeInstanceDTO, PinInstanceDTO } from '@/shared/types/dto';

// ==================== 基础事件类型 ====================

export interface BaseEvent<T = unknown> {
    type: string;
    payload: T;
}

export interface NestedEvent {
    type: string;
    payload: {
        type: string;
        payload: unknown;
    };
}

// ==================== 事件 Payload 类型 ====================

export interface ProjectLoadedPayload {
    data: ProjectData;
    path: string | null;
}

export interface ProjectSavedPayload {
    path: string;
}

export interface GraphCreatedPayload {
    id: string;
    data: Graph;
}

export interface GraphUpdatedPayload {
    id: string;
    data: Partial<Graph>;
}

export interface GraphDeletedPayload {
    id: string;
}

export interface GraphCreatedFailedPayload {
    name: string;
    error: string;
}

/** 变量创建/更新事件 payload（与后端 EventVariable 对应） */
export interface VariableCreatedPayload {
    variableId: string;
    variableScope: VariableScope;
    data: Variable;
}

export interface VariableUpdatedPayload {
    variableId: string;
    variableScope: VariableScope;
    data: Variable;
}

export interface VariableDeletedPayload {
    variableId: string;
    variableScope: VariableScope;
}

export interface DataFrameCreatedPayload {
    id: string;
    data: unknown;
}

export interface DataFrameDeletedPayload {
    id: string;
}

export interface NodeCreatedPayload {
    graphId: string;
    nodeId: string;
    data: NodeInstanceDTO;
    pins: PinInstanceDTO[];
}

export interface NodesBatchCreatedPayload {
    graphId: string;
    nodes: Array<[string, NodeInstanceDTO, PinInstanceDTO[]]>;
}

export interface NodeDeletedPayload {
    graphId: string;
    nodeId: string;
}

export interface NodesBatchDeletedPayload {
    graphId: string;
    nodeIds: string[];
}

export interface NodePositionsUpdatedPayload {
    graphId: string;
    /** [[nodeId, x, y], ...] from backend */
    updates: Array<[string, number, number]>;
}

/** 节点动态 pins 变化事件（由 PinResolver 触发） */
export interface NodePinsUpdatedPayload {
    graphId: string;
    nodeId: string;
    removedPinIds: string[];
    addedPins: PinInstanceDTO[];
    removedConnections: Array<[string, string]>;
}

/** 类型推断后 pin 的解析类型变化事件 */
export interface PinTypesInferredPayload {
    graphId: string;
    /** [pinId, resolvedType] — resolvedType 与 PinInstanceDTO.pinType 格式一致 */
    pinTypes: Array<[string, string]>;
}

// ==================== Connection 事件 Payload ====================

export interface ConnectionCreatedPayload {
    graphId: string;
    fromPin: string;
    toPin: string;
}

export interface ConnectionDeletedPayload {
    graphId: string;
    fromPin: string;
    toPin: string;
}

export interface ConnectionsBatchDeletedPayload {
    graphId: string;
    removedConnections: Array<[string, string]>;
}

// ==================== 事件处理器接口 ====================

export interface EventHandler<T = unknown> {
    eventType: string;
    handle: (payload: T, callbacks?: EventCallbacks) => void;
}

export interface EventCallbacks {
    // Project callbacks
    onProjectLoaded?: (data: ProjectData, path: string | null) => void;
    onProjectCleared?: () => void;
    onProjectSaved?: (path: string) => void;
    
    // Graph callbacks
    onEventCreated?: (id: string, data: Graph) => void;
    onFunctionCreated?: (id: string, data: Graph) => void;
    onMacroCreated?: (id: string, data: Graph) => void;
    
    // Graph error callbacks
    onEventCreatedFailed?: (name: string, error: string) => void;
    onFunctionCreatedFailed?: (name: string, error: string) => void;
    onMacroCreatedFailed?: (name: string, error: string) => void;
    
    // Variable callbacks
    onVariableCreated?: (id: string, data: Variable) => void;
    onVariableUpdated?: (id: string, data: Partial<Variable>) => void;
    onVariableDeleted?: (id: string) => void;
    
    // DataFrame callbacks
    onDataFrameCreated?: (id: string, data: unknown) => void;
    onDataFrameDeleted?: (id: string) => void;
    
    // Node callbacks
    onNodeCreated?: (graphId: string, nodeId: string, data: NodeInstanceDTO | Node) => void;
    onNodeDeleted?: (graphId: string, nodeId: string) => void;
}

// ==================== 监听器配置 ====================

export interface ListenerConfig {
    enabled?: boolean;
    callbacks?: EventCallbacks;
}

export interface ListenerInstance {
    id: string;
    unlisten: () => void;
}
