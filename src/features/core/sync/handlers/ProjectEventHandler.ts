// src/features/core/sync/handlers/ProjectEventHandler.ts

import { BaseEventHandler } from './BaseEventHandler';
import { ComputationSettingsChangedPayload, ProjectLifecycleCommittedPayload, ProjectLoadedPayload, ProjectSavedPayload } from '../types';
import { loadActivatedProject } from '@/features/core/dataStore';
import { syncApplicationEventPort } from '../applicationEventPort';
import { logger } from '@/utils/appLogger';
import { parseComputationSettingsMutationReceipt } from '@/shared/types/dto/projectComputationSettings';

const PROJECT_LIFECYCLE_EVENT_ERROR_CODE = 'project_lifecycle_protocol_error';
const SAFE_ERROR_CODE = /^[a-z][a-z0-9_]{0,63}$/;
const SAFE_INCIDENT_ID = /^[A-Za-z0-9_-]{1,128}$/;

function logProjectLifecycleEventError(error: unknown, source: string): void {
    const record = typeof error === 'object' && error !== null
        ? error as Record<string, unknown>
        : undefined;
    const code = typeof record?.code === 'string' && SAFE_ERROR_CODE.test(record.code)
        ? record.code
        : PROJECT_LIFECYCLE_EVENT_ERROR_CODE;
    const incidentId = typeof record?.incidentId === 'string'
        && SAFE_INCIDENT_ID.test(record.incidentId)
        ? record.incidentId
        : null;
    try {
        logger.sys.error(
            incidentId ? `[${code}] incidentId=${incidentId}` : `[${code}]`,
            source,
        );
    } catch {
        // Diagnostics must not control project lifecycle handling.
    }
}

export class ProjectLoadedHandler extends BaseEventHandler<ProjectLoadedPayload> {
    eventType = 'ProjectLoaded';
    
    handle(payload: ProjectLoadedPayload): void {
        this.log('Project loaded:', payload.result.path);
        void loadActivatedProject(payload.result);
    }
}

export class ProjectClearedHandler extends BaseEventHandler<void> {
    eventType = 'ProjectCleared';
    
    handle(_payload: void): void {
        this.log('Project cleared');
        const operation = syncApplicationEventPort().clearProject();
        void operation.catch((error) => {
            logProjectLifecycleEventError(error, 'ProjectClearedHandler');
        });
    }
}

export class ProjectLifecycleCommittedHandler extends BaseEventHandler<ProjectLifecycleCommittedPayload> {
    eventType = 'ProjectLifecycleCommitted';

    constructor(private readonly dependencies?: unknown) {
        super();
    }

    handle(payload: ProjectLifecycleCommittedPayload): void {
        const operation = syncApplicationEventPort().applyProjectLifecycleReceipt(
            payload.result,
            this.dependencies,
        );
        void operation.catch((error) => {
            logProjectLifecycleEventError(error, 'ProjectLifecycleCommittedHandler');
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
    
    handle(payload: ProjectSavedPayload): void {
        this.log('Project saved:', payload.result.operationId);
    }
}
