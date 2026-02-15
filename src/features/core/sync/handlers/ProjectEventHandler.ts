// src/features/core/sync/handlers/ProjectEventHandler.ts

import { BaseEventHandler } from './BaseEventHandler';
import { ProjectLoadedPayload, ProjectSavedPayload, EventCallbacks } from '../types';
import { useProjectStore } from '@/features/core/project';

export class ProjectLoadedHandler extends BaseEventHandler<ProjectLoadedPayload> {
    eventType = 'ProjectLoaded';
    
    handle(payload: ProjectLoadedPayload, callbacks?: EventCallbacks): void {
        this.log('Project loaded:', payload.path);
        
        const projectStore = useProjectStore.getState();
        projectStore.loadProject(payload.data, payload.path);
        
        callbacks?.onProjectLoaded?.(payload.data, payload.path);
    }
}

export class ProjectClearedHandler extends BaseEventHandler<void> {
    eventType = 'ProjectCleared';
    
    handle(_payload: void, callbacks?: EventCallbacks): void {
        this.log('Project cleared');
        
        const projectStore = useProjectStore.getState();
        projectStore.loadProject(
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

export class ProjectSavedHandler extends BaseEventHandler<ProjectSavedPayload> {
    eventType = 'ProjectSaved';
    
    handle(payload: ProjectSavedPayload, callbacks?: EventCallbacks): void {
        this.log('Project saved:', payload.path);
        
        const projectStore = useProjectStore.getState();
        projectStore.setCurrentPath(payload.path);
        
        callbacks?.onProjectSaved?.(payload.path);
    }
}
