// src/features/core/sync/handlers/index.ts

export * from './BaseEventHandler';
export * from './ProjectEventHandler';
export * from './GraphEventHandler';
export * from './VariableEventHandler';
export * from './DataFrameEventHandler';
export * from './ResourceEventHandler';
export * from './NodeEventHandler';
export * from './ConnectionEventHandler';

import { EventHandler } from '../types';
import {
    ProjectLoadedHandler,
    ProjectClearedHandler,
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
    GraphResourceMovedHandler,
    ProjectIndexInvalidatedHandler,
} from './ResourceEventHandler';
import {
    NodeCreatedHandler,
    NodesBatchCreatedHandler,
    NodeDeletedHandler,
    NodesBatchDeletedHandler,
    NodePositionsUpdatedHandler,
    NodePinsUpdatedHandler,
    PinTypesInferredHandler,
    RuntimeSourcesInvalidatedHandler,
} from './NodeEventHandler';
import {
    ConnectionCreatedHandler,
    ConnectionDeletedHandler,
    ConnectionsBatchDeletedHandler,
    ConnectionsBatchCreatedHandler,
} from './ConnectionEventHandler';

/**
 * 创建所有事件处理器实例
 */
export function createEventHandlers(): Array<EventHandler<unknown>> {
    return [
        // Project
        new ProjectLoadedHandler() as EventHandler<unknown>,
        new ProjectClearedHandler() as EventHandler<unknown>,
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
        new GraphResourceMovedHandler() as EventHandler<unknown>,
        
        // Node
        new NodeCreatedHandler() as EventHandler<unknown>,
        new NodesBatchCreatedHandler() as EventHandler<unknown>,
        new NodeDeletedHandler() as EventHandler<unknown>,
        new NodesBatchDeletedHandler() as EventHandler<unknown>,
        new NodePositionsUpdatedHandler() as EventHandler<unknown>,
        new NodePinsUpdatedHandler() as EventHandler<unknown>,
        new PinTypesInferredHandler() as EventHandler<unknown>,
        new RuntimeSourcesInvalidatedHandler() as EventHandler<unknown>,

        // Connection
        new ConnectionCreatedHandler() as EventHandler<unknown>,
        new ConnectionDeletedHandler() as EventHandler<unknown>,
        new ConnectionsBatchDeletedHandler() as EventHandler<unknown>,
        new ConnectionsBatchCreatedHandler() as EventHandler<unknown>,
    ];
}
