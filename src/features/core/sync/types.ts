// src/features/core/sync/types.ts

import type { ProjectData, Graph, Variable, VariableScope, Node } from '@/shared/types/domain';
import type { FunctionSignaturePin } from '@/shared/types/domain/graph';
import type { DataTypeBackendFormat } from '@/shared/types/dto/dataType';
import type { NodeInstanceDTO, PinInstanceDTO } from '@/shared/types/dto';
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
    data: ProjectData;
    path: string | null;
}

export interface ProjectSavedPayload {
    path: string;
}

export interface GraphCreatedPayload {
    path: string;
    data: Graph;
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

export interface GraphCreatedFailedPayload {
    name: string;
    error: string;
}

export interface ResourceChangedPayload {
    id: string;
    kind: ResourceKind;
    source?: 'command' | 'watcher';
    data: BackendProjectResourceMeta;
}

export interface ResourceDeletedPayload {
    id: string;
    kind: ResourceKind;
    source?: 'command' | 'watcher';
}

export interface ProjectIndexInvalidatedPayload {
    source: string;
    version: number;
}

export interface GraphResourceMovedPayload {
    from: string;
    to: string;
    kind: 'event' | 'function';
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

export interface NodeCreatedPayload {
    graphPath: string;
    nodeId: string;
    data: NodeInstanceDTO;
    pins: PinInstanceDTO[];
}

export interface NodesBatchCreatedPayload {
    graphPath: string;
    nodes: Array<[string, NodeInstanceDTO, PinInstanceDTO[]]>;
}

export interface NodeDeletedPayload {
    graphPath: string;
    nodeId: string;
}

export interface NodesBatchDeletedPayload {
    graphPath: string;
    nodeIds: string[];
}

export interface NodePositionsUpdatedPayload {
    graphPath: string;
    /** [[nodeId, x, y], ...] from backend */
    updates: Array<[string, number, number]>;
}

/** 节点动态 pins 变化事件（由 PinResolver 触发） */
export interface NodePinsUpdatedPayload {
    graphPath: string;
    nodeId: string;
    removedPinIds: string[];
    addedPins: PinInstanceDTO[];
    updatedPins?: PinInstanceDTO[];
    removedConnections: Array<[string, string]>;
    pinOrder?: string[];
}

/** 类型推断后 pin 的解析类型变化事件 */
export interface PinTypesInferredPayload {
    graphPath: string;
    pinTypes: Array<{ pinId: string; pinType: string; containerType?: string; typeDisplay?: string; dataType?: DataTypeBackendFormat }>;
}

export interface RuntimeSourcesInvalidatedPayload {
    graphPath: string;
    pinIds: string[];
}

// ==================== Backend event typing ====================

export type BackendEventType =
    | 'ProjectLoaded' | 'ProjectCleared' | 'ProjectSaved'
    | 'EventCreated' | 'EventUpdated' | 'EventDeleted' | 'EventCreatedFailed'
    | 'FunctionCreated' | 'FunctionUpdated' | 'FunctionDeleted' | 'FunctionCreatedFailed'
    | 'VariableCreated' | 'VariableUpdated' | 'VariableDeleted'
    | 'DataFrameCreated' | 'DataFrameDeleted' | 'DataFrameSchemaUpdated'
    | 'ResourceChanged' | 'ResourceDeleted' | 'GraphResourceMoved' | 'ProjectIndexInvalidated'
    | 'NodeCreated' | 'NodesBatchCreated' | 'NodeUpdated' | 'NodeDeleted' | 'NodesBatchDeleted'
    | 'NodePositionsUpdated' | 'NodePinsUpdated' | 'PinTypesInferred' | 'RuntimeSourcesInvalidated'
    | 'ConnectionCreated' | 'ConnectionDeleted' | 'ConnectionsBatchDeleted' | 'ConnectionsBatchCreated';

export interface BackendEventPayloadMap {
    ProjectLoaded: ProjectLoadedPayload;
    ProjectCleared: void;
    ProjectSaved: ProjectSavedPayload;
    EventCreated: GraphCreatedPayload;
    EventUpdated: GraphUpdatedPayload;
    EventDeleted: GraphDeletedPayload;
    EventCreatedFailed: GraphCreatedFailedPayload;
    FunctionCreated: GraphCreatedPayload;
    FunctionUpdated: GraphUpdatedPayload;
    FunctionDeleted: GraphDeletedPayload;
    FunctionCreatedFailed: GraphCreatedFailedPayload;
    VariableCreated: VariableCreatedPayload;
    VariableUpdated: VariableUpdatedPayload;
    VariableDeleted: VariableDeletedPayload;
    DataFrameCreated: DataFrameCreatedPayload;
    DataFrameDeleted: DataFrameDeletedPayload;
    DataFrameSchemaUpdated: DataFrameSchemaUpdatedPayload;
    ResourceChanged: ResourceChangedPayload;
    ResourceDeleted: ResourceDeletedPayload;
    GraphResourceMoved: GraphResourceMovedPayload;
    ProjectIndexInvalidated: ProjectIndexInvalidatedPayload;
    NodeCreated: NodeCreatedPayload;
    NodesBatchCreated: NodesBatchCreatedPayload;
    NodeUpdated: unknown;
    NodeDeleted: NodeDeletedPayload;
    NodesBatchDeleted: NodesBatchDeletedPayload;
    NodePositionsUpdated: NodePositionsUpdatedPayload;
    NodePinsUpdated: NodePinsUpdatedPayload;
    PinTypesInferred: PinTypesInferredPayload;
    RuntimeSourcesInvalidated: RuntimeSourcesInvalidatedPayload;
    ConnectionCreated: ConnectionCreatedPayload;
    ConnectionDeleted: ConnectionDeletedPayload;
    ConnectionsBatchDeleted: ConnectionsBatchDeletedPayload;
    ConnectionsBatchCreated: ConnectionsBatchCreatedPayload;
}

export type RawBackendEvent = BaseEvent | NestedEvent;

// ==================== Connection 事件 Payload ====================

export interface ConnectionCreatedPayload {
    graphPath: string;
    fromPin: string;
    toPin: string;
}

export interface ConnectionDeletedPayload {
    graphPath: string;
    fromPin: string;
    toPin: string;
}

export interface ConnectionsBatchDeletedPayload {
    graphPath: string;
    removedConnections: Array<[string, string]>;
}

export interface ConnectionsBatchCreatedPayload {
    graphPath: string;
    connections: Array<[string, string]>;
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
    
    // Graph error callbacks
    onEventCreatedFailed?: (name: string, error: string) => void;
    onFunctionCreatedFailed?: (name: string, error: string) => void;
    
    // Variable callbacks
    onVariableCreated?: (id: string, data: Variable) => void;
    onVariableUpdated?: (id: string, data: Partial<Variable>) => void;
    onVariableDeleted?: (id: string) => void;
    
    // DataFrame callbacks
    onDataFrameCreated?: (id: string, data: unknown) => void;
    onDataFrameDeleted?: (id: string) => void;
    
    // Node callbacks
    onNodeCreated?: (graphPath: string, nodeId: string, data: NodeInstanceDTO | Node) => void;
    onNodeDeleted?: (graphPath: string, nodeId: string) => void;
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
