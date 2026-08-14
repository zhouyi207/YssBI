import { syncApplicationEventPort } from '../applicationEventPort';
import { pendingMutationGraphPath } from '@/features/core/history/pendingMutationPort';
import { useGraphDataStore } from '@/features/core/dataStore/graphDataStore';
import { useNodeCatalogStore } from '@/features/core/nodeCatalog/nodeCatalogStore';

import type { GraphDeltaDto } from '@/shared/types/dto/editorMutation';
import { BaseEventHandler } from './BaseEventHandler';
import type {
  GraphDeltaEventPayload,
  ResourceMutationCommittedPayload,
} from '../types';
import {
  captureProjectIdentity,
  isCurrentProjectIdentity,
  type ProjectIdentitySnapshot,
} from '@/features/core/projectLifecycle/projectLifecycleAuthority';
import {
  parseGraphDeltaEventPayload,
  parseResourceMutationCommittedPayload,
} from '../utils/projectEventWireParser';
import { inferGraphResourceKind } from '@/shared/types/domain/graphResourcePath';
import { markResourceStale } from '@/features/core/resource';

function captureCurrentProjectEventIdentity(
  projectInstanceId: string,
): ProjectIdentitySnapshot | null {
  try {
    const identity = captureProjectIdentity();
    return identity.projectInstanceId === projectInstanceId
      && isCurrentProjectIdentity(identity)
      ? identity
      : null;
  } catch {
    return null;
  }
}

function isCurrentProjectEvent(projectInstanceId: string): boolean {
  return captureCurrentProjectEventIdentity(projectInstanceId) !== null;
}

function markMalformedCurrentGraphEventStale(payload: unknown): void {
  if (typeof payload !== 'object' || payload === null || Array.isArray(payload)) return;
  const projectInstanceId = (payload as Record<string, unknown>).projectInstanceId;
  const delta = (payload as Record<string, unknown>).delta;
  if (typeof projectInstanceId !== 'string'
    || !isCurrentProjectEvent(projectInstanceId)
    || typeof delta !== 'object'
    || delta === null
    || Array.isArray(delta)) return;
  const graphPath = (delta as Record<string, unknown>).graphPath;
  if (typeof graphPath !== 'string') return;
  const kind = inferGraphResourceKind(graphPath);
  if (kind) markResourceStale({ id: graphPath, kind }, true);
}

export class GraphDeltaHandler extends BaseEventHandler<GraphDeltaEventPayload> {
  eventType = 'GraphDelta';

  handle(payload: GraphDeltaEventPayload): void {
    let parsed: GraphDeltaEventPayload;
    try {
      parsed = parseGraphDeltaEventPayload(payload);
    } catch {
      markMalformedCurrentGraphEventStale(payload);
      return;
    }
    const delta: GraphDeltaDto = parsed.delta;
    const identity = captureCurrentProjectEventIdentity(parsed.projectInstanceId);
    if (!identity) return;

    const pendingGraphPath = delta.causedBy ? pendingMutationGraphPath(delta.causedBy) : undefined;
    if (pendingGraphPath === delta.graphPath) return;

    const current = useGraphDataStore.getState().graphEntities[delta.graphPath];
    if (current && delta.toRevision <= current.sourceRevision) return;
    if (!isCurrentProjectIdentity(identity)) return;

    syncApplicationEventPort().graphDelta(delta.graphPath);
  }
}

export class ResourceMutationCommittedHandler extends BaseEventHandler<ResourceMutationCommittedPayload> {
  eventType = 'ResourceMutationCommitted';

  handle(payload: ResourceMutationCommittedPayload): void {
    let parsed: ResourceMutationCommittedPayload;
    try {
      parsed = parseResourceMutationCommittedPayload(payload);
    } catch {
      return;
    }
    const committed = parsed.result;
    const identity = captureCurrentProjectEventIdentity(committed.projectInstanceId);
    if (!identity) return;
    void syncApplicationEventPort().resourceMutationCommitted(committed).then(() => {
      if (!isCurrentProjectIdentity(identity)) return;
      useNodeCatalogStore.getState().observeResourcePublication(
        committed.projectInstanceId,
        committed.publicationRevision,
      );
    }).catch((error) => {
      this.error(
        'Resource publication event failed:',
        error instanceof Error ? error.message : String(error),
      );
    });
  }
}
