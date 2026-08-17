// src/features/core/sync/handlers/index.ts

export * from './BaseEventHandler';
export * from './ProjectEventHandler';
export * from './ResourceEventHandler';
export * from './ProjectMutationEventHandler';

import { EventHandler } from '../types';
import {
    ProjectLoadedHandler,
    ProjectClearedHandler,
    ProjectLifecycleCommittedHandler,
    ProjectSavedHandler,
    ComputationSettingsChangedHandler,
} from './ProjectEventHandler';
import { ProjectIndexInvalidatedHandler } from './ResourceEventHandler';
import {
    GraphDeltaHandler,
    ResourceMutationCommittedHandler,
} from './ProjectMutationEventHandler';

/**
 * 创建所有事件处理器实例
 */
export function createEventHandlers(): Array<EventHandler<unknown>> {
    return [
        // Project
        new ProjectLoadedHandler() as EventHandler<unknown>,
        new ProjectClearedHandler() as EventHandler<unknown>,
        new ProjectLifecycleCommittedHandler() as EventHandler<unknown>,
        new ProjectSavedHandler() as EventHandler<unknown>,
        new ComputationSettingsChangedHandler() as EventHandler<unknown>,

        // Resource
        new ProjectIndexInvalidatedHandler() as EventHandler<unknown>,

        // Revisioned project mutations
        new GraphDeltaHandler() as EventHandler<unknown>,
        new ResourceMutationCommittedHandler() as EventHandler<unknown>,
    ];
}
