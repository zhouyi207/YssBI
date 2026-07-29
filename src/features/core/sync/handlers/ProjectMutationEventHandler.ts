import { projectPublicationCoordinator } from '@/features/application/editorMutation/projectPublicationCoordinator';
import { getPendingMutation } from '@/features/application/editorMutation/pendingMutationRegistry';
import { useGraphDataStore } from '@/features/core/dataStore/graphDataStore';
import { invalidateGraphProjection } from '@/features/application/editorProjection/graphProjectionCoordinator';
import type { GraphDeltaDto, ResourceMutationResultDto } from '@/shared/types/dto/editorMutation';
import { BaseEventHandler } from './BaseEventHandler';
import type {
  GraphDeltaEventPayload,
  ResourceMutationCommittedPayload,
} from '../types';
import {
  captureProjectIdentity,
  isCurrentProjectIdentity,
} from '@/services/project/projectIdentity';

function isCurrentProjectEvent(projectInstanceId: string): boolean {
  try {
    const identity = captureProjectIdentity();
    return identity.projectInstanceId === projectInstanceId
      && isCurrentProjectIdentity(identity);
  } catch {
    return false;
  }
}

export class GraphDeltaHandler extends BaseEventHandler<GraphDeltaEventPayload> {
  eventType = 'GraphDelta';

  handle(payload: GraphDeltaEventPayload): void {
    const delta: GraphDeltaDto | undefined = payload?.delta;
    if (!delta || typeof delta.graphPath !== 'string') return;
    if (!isCurrentProjectEvent(payload.projectInstanceId)) return;

    const pending = delta.causedBy ? getPendingMutation(delta.causedBy) : undefined;
    if (pending?.graphPath === delta.graphPath) return;

    const current = useGraphDataStore.getState().graphEntities[delta.graphPath];
    if (current && delta.toRevision <= current.sourceRevision) return;

    void invalidateGraphProjection(delta.graphPath);
  }
}

export class ResourceMutationCommittedHandler extends BaseEventHandler<ResourceMutationCommittedPayload> {
  eventType = 'ResourceMutationCommitted';

  handle(payload: ResourceMutationCommittedPayload): void {
    const result: unknown = payload?.result;
    if (!result || typeof result !== 'object') return;
    const projectInstanceId = (result as { projectInstanceId?: unknown }).projectInstanceId;
    if (typeof projectInstanceId !== 'string' || !isCurrentProjectEvent(projectInstanceId)) return;
    void projectPublicationCoordinator.submit({
      result: result as ResourceMutationResultDto,
    }).catch((error) => {
      this.error(
        'Resource publication event failed:',
        error instanceof Error ? error.message : String(error),
      );
    });
  }
}
