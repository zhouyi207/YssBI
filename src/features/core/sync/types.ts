// src/features/core/sync/types.ts

import { ProjectData, GraphData, VariableData } from '@/shared/types/domain';

// ==================== 基础事件类型 ====================

export interface BaseEvent<T = any> {
    type: string;
    payload: T;
}

export interface NestedEvent {
    type: string;
    payload: {
        type: string;
        payload: any;
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
    data: GraphData;
}

export interface GraphUpdatedPayload {
    id: string;
    data: Partial<GraphData>;
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
    data: VariableData;
}

export interface VariableUpdatedPayload {
    id: string;
    data: Partial<VariableData>;
}

export interface VariableDeletedPayload {
    id: string;
}

export interface DataFrameCreatedPayload {
    id: string;
    data: any;
}

export interface DataFrameDeletedPayload {
    id: string;
}

export interface NodeCreatedPayload {
    graph_id: string;
    node_id: string;
    data: any; // NodeInstanceDTO from backend
}

export interface NodeDeletedPayload {
    graph_id: string;
    node_id: string;
}

// ==================== 事件处理器接口 ====================

export interface EventHandler<T = any> {
    eventType: string;
    handle: (payload: T, callbacks?: EventCallbacks) => void;
}

export interface EventCallbacks {
    // Project callbacks
    onProjectLoaded?: (data: ProjectData, path: string | null) => void;
    onProjectCleared?: () => void;
    onProjectSaved?: (path: string) => void;
    
    // Graph callbacks
    onEventCreated?: (id: string, data: GraphData) => void;
    onFunctionCreated?: (id: string, data: GraphData) => void;
    onMacroCreated?: (id: string, data: GraphData) => void;
    
    // Graph error callbacks
    onEventCreatedFailed?: (name: string, error: string) => void;
    onFunctionCreatedFailed?: (name: string, error: string) => void;
    onMacroCreatedFailed?: (name: string, error: string) => void;
    
    // Variable callbacks
    onVariableCreated?: (id: string, data: VariableData) => void;
    onVariableUpdated?: (id: string, data: Partial<VariableData>) => void;
    onVariableDeleted?: (id: string) => void;
    
    // DataFrame callbacks
    onDataFrameCreated?: (id: string, data: any) => void;
    onDataFrameDeleted?: (id: string) => void;
    
    // Node callbacks
    onNodeCreated?: (graphId: string, nodeId: string, data: any) => void;
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
