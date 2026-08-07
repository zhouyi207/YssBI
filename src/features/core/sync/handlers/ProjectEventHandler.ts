// src/features/core/sync/handlers/ProjectEventHandler.ts

import { BaseEventHandler } from './BaseEventHandler';
import { ProjectLifecycleCommittedPayload, ProjectLoadedPayload, ProjectSavedPayload, EventCallbacks } from '../types';
import { loadActivatedProject } from '@/features/core/dataStore';
import {
    applyProjectLifecycleReceipt,
    type ProjectLifecycleReceiptDependencies,
} from '@/features/application/projectLifecycleReceipt';
import { createProjectLifecycleReceiptDependencies } from '@/features/application/projectLifecycleReceiptDependencies';
import { formatErrorMessage } from '@/shared/utils/formatErrorMessage';
import { logger } from '@/utils/appLogger';

export class ProjectLoadedHandler extends BaseEventHandler<ProjectLoadedPayload> {
    eventType = 'ProjectLoaded';
    
    handle(payload: ProjectLoadedPayload, callbacks?: EventCallbacks): void {
        this.log('Project loaded:', payload.result.path);

        loadActivatedProject(payload.result).then((data) => {
            if (data) callbacks?.onProjectLoaded?.(data, payload.result.path);
        });
    }
}

export class ProjectClearedHandler extends BaseEventHandler<void> {
    eventType = 'ProjectCleared';
    
    handle(_payload: void, callbacks?: EventCallbacks): void {
        this.log('Project cleared');
        
        createProjectLifecycleReceiptDependencies(
            (callbacks ?? this.callbacks)?.onProjectCleared,
        ).clearProject();
    }
}

export class ProjectLifecycleCommittedHandler extends BaseEventHandler<ProjectLifecycleCommittedPayload> {
    eventType = 'ProjectLifecycleCommitted';

    constructor(
        callbacks?: EventCallbacks,
        private readonly dependencies?: ProjectLifecycleReceiptDependencies,
    ) {
        super(callbacks);
    }

    handle(payload: ProjectLifecycleCommittedPayload, callbacks?: EventCallbacks): void {
        void applyProjectLifecycleReceipt(
            payload.result,
            'event',
            this.dependencies ?? createProjectLifecycleReceiptDependencies(
                (callbacks ?? this.callbacks)?.onProjectCleared,
            ),
        ).catch((error) => {
            logger.sys.error(
                `Project lifecycle event failed: ${formatErrorMessage(error)}`,
                'ProjectLifecycleCommittedHandler',
            );
        });
    }
}

export class ProjectSavedHandler extends BaseEventHandler<ProjectSavedPayload> {
    eventType = 'ProjectSaved';
    
    handle(payload: ProjectSavedPayload, _callbacks?: EventCallbacks): void {
        this.log('Project saved:', payload.result.operationId);
    }
}
