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
} from './NodeEventHandler';

/**
 * 创建所有事件处理器实例
 */
export function createEventHandlers(): EventHandler[] {
    return [
        // Project
        new ProjectLoadedHandler(),
        new ProjectClearedHandler(),
        new ProjectSavedHandler(),
        
        // Event
        new EventCreatedHandler(),
        new EventUpdatedHandler(),
        new EventDeletedHandler(),
        new EventCreatedFailedHandler(),
        
        // Function
        new FunctionCreatedHandler(),
        new FunctionUpdatedHandler(),
        new FunctionDeletedHandler(),
        new FunctionCreatedFailedHandler(),
        
        // Macro
        new MacroCreatedHandler(),
        new MacroUpdatedHandler(),
        new MacroDeletedHandler(),
        new MacroCreatedFailedHandler(),
        
        // Variable
        new VariableCreatedHandler(),
        new VariableUpdatedHandler(),
        new VariableDeletedHandler(),
        
        // DataFrame
        new DataFrameCreatedHandler(),
        new DataFrameDeletedHandler(),
        
        // Node
        new NodeCreatedHandler(),
        new NodeDeletedHandler(),
    ];
}
