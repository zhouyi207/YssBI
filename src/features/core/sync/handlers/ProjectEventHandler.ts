// src/features/core/sync/handlers/ProjectEventHandler.ts

import { BaseEventHandler } from './BaseEventHandler';
import { ComputationSettingsChangedPayload, ProjectLifecycleCommittedPayload, ProjectLoadedPayload, ProjectSavedPayload, EventCallbacks } from '../types';
import { loadActivatedProject } from '@/features/core/dataStore';
import { syncApplicationEventPort } from '../applicationEventPort';
import { formatErrorMessage } from '@/shared/utils/formatErrorMessage';
import { logger } from '@/utils/appLogger';
import { parseComputationSettingsMutationReceipt } from '@/shared/types/dto/projectComputationSettings';

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
        
        syncApplicationEventPort().clearProject(
            (callbacks ?? this.callbacks)?.onProjectCleared,
        );
    }
}

export class ProjectLifecycleCommittedHandler extends BaseEventHandler<ProjectLifecycleCommittedPayload> {
    eventType = 'ProjectLifecycleCommitted';

    constructor(
        callbacks?: EventCallbacks,
        private readonly dependencies?: unknown,
    ) {
        super(callbacks);
    }

    handle(payload: ProjectLifecycleCommittedPayload, callbacks?: EventCallbacks): void {
        const operation = syncApplicationEventPort().applyProjectLifecycleReceipt(
            payload.result,
            (callbacks ?? this.callbacks)?.onProjectCleared,
            this.dependencies,
        );
        void operation.catch((error) => {
            logger.sys.error(
                `Project lifecycle event failed: ${formatErrorMessage(error)}`,
                'ProjectLifecycleCommittedHandler',
            );
        });
    }
}

export class ComputationSettingsChangedHandler extends BaseEventHandler<ComputationSettingsChangedPayload> {
    eventType = 'ComputationSettingsChanged';

    handle(payload: ComputationSettingsChangedPayload): void {
        syncApplicationEventPort().computationSettingsChanged(
            parseComputationSettingsMutationReceipt(payload.result),
        );
    }
}

export class ProjectSavedHandler extends BaseEventHandler<ProjectSavedPayload> {
    eventType = 'ProjectSaved';
    
    handle(payload: ProjectSavedPayload, _callbacks?: EventCallbacks): void {
        this.log('Project saved:', payload.result.operationId);
    }
}
