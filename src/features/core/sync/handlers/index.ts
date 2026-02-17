// src/features/core/sync/handlers/index.ts

export * from './BaseEventHandler';
export * from './ProjectEventHandler';
export * from './GraphEventHandler';
export * from './VariableEventHandler';
export * from './DataFrameEventHandler';
export * from './NodeEventHandler';

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
    MacroCreatedHandler,
    MacroUpdatedHandler,
    MacroDeletedHandler,
    MacroCreatedFailedHandler,
} from './GraphEventHandler';
import {
    VariableCreatedHandler,
    VariableUpdatedHandler,
    VariableDeletedHandler,
} from './VariableEventHandler';
import {
    DataFrameCreatedHandler,
    DataFrameDeletedHandler,
} from './DataFrameEventHandler';
import {
    NodeCreatedHandler,
    NodeDeletedHandler,
    NodePositionsUpdatedHandler,
} from './NodeEventHandler';

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
        
        // Macro
        new MacroCreatedHandler() as EventHandler<unknown>,
        new MacroUpdatedHandler() as EventHandler<unknown>,
        new MacroDeletedHandler() as EventHandler<unknown>,
        new MacroCreatedFailedHandler() as EventHandler<unknown>,
        
        // Variable
        new VariableCreatedHandler() as EventHandler<unknown>,
        new VariableUpdatedHandler() as EventHandler<unknown>,
        new VariableDeletedHandler() as EventHandler<unknown>,
        
        // DataFrame
        new DataFrameCreatedHandler() as EventHandler<unknown>,
        new DataFrameDeletedHandler() as EventHandler<unknown>,
        
        // Node
        new NodeCreatedHandler() as EventHandler<unknown>,
        new NodeDeletedHandler() as EventHandler<unknown>,
        new NodePositionsUpdatedHandler() as EventHandler<unknown>,
    ];
}
