import { BaseEventHandler } from './BaseEventHandler';
import type { ProjectIndexInvalidatedPayload } from '../types';
import { notifyIndexInvalidated } from '@/features/core/resource';
import {
  captureProjectIdentity,
  isCurrentProjectIdentity,
  type ProjectIdentitySnapshot,
} from '@/features/core/projectLifecycle/projectLifecycleAuthority';
import { parseProjectIndexInvalidatedPayload } from '../utils/projectEventWireParser';

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

export class ProjectIndexInvalidatedHandler extends BaseEventHandler<ProjectIndexInvalidatedPayload> {
  eventType = 'ProjectIndexInvalidated';

  handle(payload: ProjectIndexInvalidatedPayload): void {
    let parsed: ProjectIndexInvalidatedPayload;
    try {
      parsed = parseProjectIndexInvalidatedPayload(payload);
    } catch {
      return;
    }
    const identity = captureCurrentProjectEventIdentity(parsed.projectInstanceId);
    if (!identity) return;
    this.log('Project index invalidated:', parsed.source, parsed.version);
    void notifyIndexInvalidated(identity, parsed.version);
  }
}
