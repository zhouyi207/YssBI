// src/features/core/sync/types.ts

import { ProjectData, Graph, Variable, Node } from '@/shared/types/domain';
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

export interface VariableCreatedPayload {
    id: string;
    data: Variable;
}

export interface VariableUpdatedPayload {
    id: string;
    data: Partial<Variable>;
}

export interface VariableDeletedPayload {
    id: string;
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

export interface NodeDeletedPayload {
    graphId: string;
    nodeId: string;
}

export interface NodePositionsUpdatedPayload {
    graphId: string;
    /** [[nodeId, x, y], ...] from backend */
    updates: Array<[string, number, number]>;
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
