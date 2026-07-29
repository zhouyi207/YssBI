// src/features/core/sync/handlers/ProjectEventHandler.ts

import { BaseEventHandler } from './BaseEventHandler';
import { ProjectLifecycleCommittedPayload, ProjectLoadedPayload, ProjectSavedPayload, EventCallbacks } from '../types';
import { useProjectIOStore } from '@/features/core/dataStore';
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
        this.log('Project loaded:', payload.path);
        
        // 前端只传路径，后端负责加载；收到事件后从后端按当前分阶段加载流程同步状态。
        useProjectIOStore.getState().loadProject().then((data) => {
            if (data) callbacks?.onProjectLoaded?.(data, payload.path);
        });
    }
}

export class ProjectClearedHandler extends BaseEventHandler<void> {
    eventType = 'ProjectCleared';
    
    handle(_payload: void, callbacks?: EventCallbacks): void {
        this.log('Project cleared');
        
        useProjectIOStore.getState().loadProjectFromData(
            {
                variables: {},
                graphs: {},
                databases: {},
                metadata: { exportTime: '', appVersion: '' },
            },
            null
        );
        
        callbacks?.onProjectCleared?.();
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
