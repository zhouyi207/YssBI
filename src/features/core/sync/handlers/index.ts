// src/features/core/sync/handlers/index.ts

export * from './BaseEventHandler';
export * from './ProjectEventHandler';
export * from './GraphEventHandler';
export * from './VariableEventHandler';
export * from './DataFrameEventHandler';
export * from './ResourceEventHandler';
export * from './ProjectMutationEventHandler';

import { EventHandler } from '../types';
import {
    ProjectLoadedHandler,
    ProjectClearedHandler,
    ProjectLifecycleCommittedHandler,
    ProjectSavedHandler,
} from './ProjectEventHandler';
import {
    EventUpdatedHandler,
    EventDeletedHandler,
    FunctionUpdatedHandler,
    FunctionDeletedHandler,
} from './GraphEventHandler';
import {
    VariableCreatedHandler,
    VariableUpdatedHandler,
    VariableDeletedHandler,
} from './VariableEventHandler';
import {
    DataFrameCreatedHandler,
    DataFrameDeletedHandler,
    DataFrameSchemaUpdatedHandler,
} from './DataFrameEventHandler';
import {
    ResourceChangedHandler,
    ProjectIndexInvalidatedHandler,
} from './ResourceEventHandler';
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
        
        // Event
        new EventUpdatedHandler() as EventHandler<unknown>,
        new EventDeletedHandler() as EventHandler<unknown>,
        
        // Function
        new FunctionUpdatedHandler() as EventHandler<unknown>,
        new FunctionDeletedHandler() as EventHandler<unknown>,
        
        // Variable
        new VariableCreatedHandler() as EventHandler<unknown>,
        new VariableUpdatedHandler() as EventHandler<unknown>,
        new VariableDeletedHandler() as EventHandler<unknown>,
        
        // DataFrame
        new DataFrameCreatedHandler() as EventHandler<unknown>,
        new DataFrameDeletedHandler() as EventHandler<unknown>,
        new DataFrameSchemaUpdatedHandler() as EventHandler<unknown>,

        // Resource
        new ProjectIndexInvalidatedHandler() as EventHandler<unknown>,
        new ResourceChangedHandler() as EventHandler<unknown>,

        // Revisioned project mutations
        new GraphDeltaHandler() as EventHandler<unknown>,
        new ResourceMutationCommittedHandler() as EventHandler<unknown>,
    ];
}
