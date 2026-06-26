// src/features/core/sync/handlers/index.ts

export * from './BaseEventHandler';
export * from './ProjectEventHandler';
export * from './GraphEventHandler';
export * from './VariableEventHandler';
export * from './DataFrameEventHandler';
export * from './NodeEventHandler';
export * from './ConnectionEventHandler';

import { EventHandler } from '../types';
import {
    ProjectLoadedHandler,
    ProjectClearedHandler,
    ProjectSavedHandler,
} from './ProjectEventHandler';
import {
    EventCreatedHandler,
    EventUpdatedHandler,
    EventDeletedHandler,
    EventCreatedFailedHandler,
    FunctionCreatedHandler,
    FunctionUpdatedHandler,
    FunctionDeletedHandler,
    FunctionCreatedFailedHandler,
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
    NodeCreatedHandler,
    NodesBatchCreatedHandler,
    NodeDeletedHandler,
    NodesBatchDeletedHandler,
    NodePositionsUpdatedHandler,
    NodePinsUpdatedHandler,
    PinTypesInferredHandler,
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
        new EventCreatedHandler() as EventHandler<unknown>,
        new EventUpdatedHandler() as EventHandler<unknown>,
        new EventDeletedHandler() as EventHandler<unknown>,
        new EventCreatedFailedHandler() as EventHandler<unknown>,
        
        // Function
        new FunctionCreatedHandler() as EventHandler<unknown>,
        new FunctionUpdatedHandler() as EventHandler<unknown>,
        new FunctionDeletedHandler() as EventHandler<unknown>,
        new FunctionCreatedFailedHandler() as EventHandler<unknown>,
        
        // Variable
        new VariableCreatedHandler() as EventHandler<unknown>,
        new VariableUpdatedHandler() as EventHandler<unknown>,
        new VariableDeletedHandler() as EventHandler<unknown>,
        
        // DataFrame
        new DataFrameCreatedHandler() as EventHandler<unknown>,
        new DataFrameDeletedHandler() as EventHandler<unknown>,
        new DataFrameSchemaUpdatedHandler() as EventHandler<unknown>,
        
        // Node
        new NodeCreatedHandler() as EventHandler<unknown>,
        new NodesBatchCreatedHandler() as EventHandler<unknown>,
        new NodeDeletedHandler() as EventHandler<unknown>,
        new NodesBatchDeletedHandler() as EventHandler<unknown>,
        new NodePositionsUpdatedHandler() as EventHandler<unknown>,
        new NodePinsUpdatedHandler() as EventHandler<unknown>,
        new PinTypesInferredHandler() as EventHandler<unknown>,

        // Connection
        new ConnectionCreatedHandler() as EventHandler<unknown>,
        new ConnectionDeletedHandler() as EventHandler<unknown>,
        new ConnectionsBatchDeletedHandler() as EventHandler<unknown>,
        new ConnectionsBatchCreatedHandler() as EventHandler<unknown>,
    ];
}
