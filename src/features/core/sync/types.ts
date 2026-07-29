// src/features/core/sync/types.ts

import type { ProjectData, Graph, Variable, VariableScope } from '@/shared/types/domain';
import type { FunctionSignaturePin } from '@/shared/types/domain/graph';
import type {
    GraphDeltaDto,
    ResourceMutationResultDto,
} from '@/shared/types/dto/editorMutation';
import type { BackendProjectResourceMeta, ResourceKind } from '@/features/core/resource';

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
    path: string | null;
}

export interface ProjectLifecycleCommittedPayload {
    result: import('@/shared/types/dto/project').LifecycleMutationResultDto;
}

export interface ProjectSavedPayload {
    result: import('@/shared/types/dto').ProjectSaveResultDto;
}

export interface GraphUpdatedPayload {
    path: string;
    data: Partial<Graph> & {
        functionInputs?: FunctionSignaturePin[];
        functionOutputs?: FunctionSignaturePin[];
    };
}

export interface GraphDeletedPayload {
    path: string;
}

export interface ResourceChangedPayload {
    projectInstanceId: string;
    id: string;
    kind: ResourceKind;
    source?: 'command' | 'watcher';
    data: BackendProjectResourceMeta;
}

export interface ProjectIndexInvalidatedPayload {
    projectInstanceId: string;
    source: string;
    version: number;
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

/** 后端某个 SQL/Excel 数据源完成异步物化后回填 schema（与后端 `EventDataframe::DataFrameSchemaUpdated` 对齐）。 */
export interface DataFrameColumnInfo {
    name: string;
    /** 后端字段名为 `type`，此处映射后保持一致。 */
    type: string;
}

export interface DataFrameSchemaUpdatedPayload {
    id: string;
    columns: DataFrameColumnInfo[];
    rowCount: number;
    columnCount: number;
    /** 物化失败时的错误信息；与 columns/rowCount 互斥。 */
    error?: string;
}

export interface GraphDeltaEventPayload {
    projectInstanceId: string;
    delta: GraphDeltaDto;
}

export interface ResourceMutationCommittedPayload {
    result: ResourceMutationResultDto;
}

// ==================== Backend event typing ====================

export type BackendEventType =
    | 'ProjectLoaded' | 'ProjectCleared' | 'ProjectLifecycleCommitted' | 'ProjectSaved'
    | 'EventUpdated' | 'EventDeleted'
    | 'FunctionUpdated' | 'FunctionDeleted'
    | 'VariableCreated' | 'VariableUpdated' | 'VariableDeleted'
    | 'DataFrameCreated' | 'DataFrameDeleted' | 'DataFrameSchemaUpdated'
    | 'ResourceChanged' | 'ProjectIndexInvalidated'
    | 'GraphDelta' | 'ResourceMutationCommitted';

export interface BackendEventPayloadMap {
    ProjectLoaded: ProjectLoadedPayload;
    ProjectCleared: void;
    ProjectLifecycleCommitted: ProjectLifecycleCommittedPayload;
    ProjectSaved: ProjectSavedPayload;
    EventUpdated: GraphUpdatedPayload;
    EventDeleted: GraphDeletedPayload;
    FunctionUpdated: GraphUpdatedPayload;
    FunctionDeleted: GraphDeletedPayload;
    VariableCreated: VariableCreatedPayload;
    VariableUpdated: VariableUpdatedPayload;
    VariableDeleted: VariableDeletedPayload;
    DataFrameCreated: DataFrameCreatedPayload;
    DataFrameDeleted: DataFrameDeletedPayload;
    DataFrameSchemaUpdated: DataFrameSchemaUpdatedPayload;
    ResourceChanged: ResourceChangedPayload;
    ProjectIndexInvalidated: ProjectIndexInvalidatedPayload;
    GraphDelta: GraphDeltaEventPayload;
    ResourceMutationCommitted: ResourceMutationCommittedPayload;
}

export type RawBackendEvent = BaseEvent | NestedEvent;



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
    
    // Variable callbacks
    onVariableCreated?: (id: string, data: Variable) => void;
    onVariableUpdated?: (id: string, data: Partial<Variable>) => void;
    onVariableDeleted?: (id: string) => void;
    
    // DataFrame callbacks
    onDataFrameCreated?: (id: string, data: unknown) => void;
    onDataFrameDeleted?: (id: string) => void;
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
