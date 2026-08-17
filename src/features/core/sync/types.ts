// src/features/core/sync/types.ts

import type {
    GraphDeltaDto,
    ResourceMutationResultDto,
} from '@/shared/types/dto/editorMutation';


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
    result: {
        path: string;
        projectInstanceId: string;
        activationRevision: number;
    };
}

export interface ProjectLifecycleCommittedPayload {
    result: import('@/shared/types/dto/project').LifecycleMutationResultDto;
}

export interface ProjectSavedPayload {
    result: import('@/shared/types/dto').ProjectSaveResultDto;
}

export interface ComputationSettingsChangedPayload {
    result: import('@/shared/types/dto/projectComputationSettings').ComputationSettingsMutationReceiptDto;
}

export interface ProjectIndexInvalidatedPayload {
    projectInstanceId: string;
    source: 'watcher';
    version: number;
}

export interface GraphDeltaEventPayload {
    projectInstanceId: string;
    delta: GraphDeltaDto;
}

export interface ResourceMutationCommittedPayload {
    result: ResourceMutationResultDto;
}


export type RawBackendEvent = BaseEvent | NestedEvent;



// ==================== 事件处理器接口 ====================

export interface EventHandler<T = unknown> {
    eventType: string;
    handle: (payload: T) => void;
}
