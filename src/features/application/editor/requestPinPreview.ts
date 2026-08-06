import type { TFunction } from 'i18next';
import { useGraphDataStore } from '@/features/core/dataStore/graphDataStore';
import type { GraphEntityBucket } from '@/features/core/dataStore/graphEntityAccess';
import { isGraphCachedInMemory } from '@/features/core/dataStore/graphDocumentLoadPolicy';
import { useGraphSessionStore } from '@/features/core/graphSession/graphSessionStore';
import {
  assertCurrentProjectIdentity,
  captureProjectIdentity,
  isCurrentProjectIdentity,
  type ProjectIdentitySnapshot,
} from '@/features/core/projectLifecycle/projectLifecycleAuthority';
import {
  lookupPinPreview,
  useExecutionStore,
} from '@/features/core/execution';
import { uiStore } from '@/features/core/ui/UIStore';
import { ProjectService } from '@/services/project/projectService';
import { openInspectableSource } from '@/features/application/execution/openInspectableSource';
import { windowSourceRef } from '@/features/core/resultSource';
import type { PortAddressDto } from '@/shared/types/dto/editorProjection';
import type { GraphOutputRefDto } from '@/shared/types/dto/executionDemand';
import type { PinData } from '@/shared/types/store/graph';
import { inferGraphResourceKind } from '@/shared/types/domain/graphResourcePath';
import { formatErrorMessage } from '@/shared/utils/formatErrorMessage';
import {
  observeGraphRunEvent,
  type GraphRunOutcomeState,
  type PinPreviewObservation,
} from './observeGraphRunEvent';

export type PinPreviewRejectionReason =
  | 'nested-function'
  | 'missing-session'
  | 'missing-resource'
  | 'missing-pin'
  | 'missing-address'
  | 'input-pin'
  | 'non-data-output'
  | 'orphan-pin'
  | 'stale-project-lifecycle';

export type PinPreviewRequestResult =
  | { status: 'completed'; generation: number; sourceId: string }
  | { status: 'rejected'; reason: PinPreviewRejectionReason }
  | { status: 'failed'; generation: number; error: string };

type PreviewAuthority = {
  project: ProjectIdentitySnapshot;
  projection: GraphEntityBucket;
  requestGeneration: number;
  sourceRevision: number;
};

export function isPinPreviewActionAvailable(
  graphPath: string | undefined,
  pin: Pick<PinData, 'direction' | 'kind' | 'address' | 'orphan' | 'status'>,
): boolean {
  return Boolean(
    graphPath
    && inferGraphResourceKind(graphPath) === 'event'
    && pin.direction === 'output'
    && pin.kind === 'data'
    && pin.address
    && !pin.orphan
    && pin.status !== 'orphan',
  );
}

type ValidPreviewRequest = {
  output: GraphOutputRefDto;
  authority: PreviewAuthority;
};

function reject(reason: PinPreviewRejectionReason): PinPreviewRequestResult {
  uiStore.showToast(`无法预览此输出：${reason}`, 'warning', 3000);
  return { status: 'rejected', reason };
}

function validatePin(pin: PinData | undefined): PinPreviewRejectionReason | null {
  if (!pin) return 'missing-pin';
  if (!pin.address) return 'missing-address';
  if (pin.direction !== 'output') return 'input-pin';
  if (pin.kind !== 'data') return 'non-data-output';
  if (pin.orphan || pin.status === 'orphan') return 'orphan-pin';
  return null;
}

function capturePreviewRequest(
  graphPath: string,
  pinId: string,
): ValidPreviewRequest | PinPreviewRejectionReason {
  let project: ProjectIdentitySnapshot;
  try {
    project = captureProjectIdentity();
  } catch {
    return 'stale-project-lifecycle';
  }

  const graphKind = inferGraphResourceKind(graphPath);
  if (graphKind === 'function') return 'nested-function';
  if (graphKind !== 'event') return 'missing-resource';
  if (!useGraphSessionStore.getState().isFocusedGraphPath(graphPath)) return 'missing-session';
  if (!isGraphCachedInMemory(graphPath)) return 'missing-resource';

  const bucket = useGraphDataStore.getState().graphEntities[graphPath];
  if (!bucket) return 'missing-resource';
  const pin = bucket.pins[pinId];
  const invalid = validatePin(pin);
  if (invalid) return invalid;

  return {
    output: { graphPath, port: pin.address as PortAddressDto },
    authority: {
      project,
      projection: bucket,
      requestGeneration: bucket.requestGeneration,
      sourceRevision: bucket.sourceRevision,
    },
  };
}

function isPreviewAuthorityCurrent(graphPath: string, authority: PreviewAuthority): boolean {
  if (!isCurrentProjectIdentity(authority.project) || !isGraphCachedInMemory(graphPath)) {
    return false;
  }
  const bucket = useGraphDataStore.getState().graphEntities[graphPath];
  return Boolean(
    bucket === authority.projection
    && bucket.requestGeneration === authority.requestGeneration
    && bucket.sourceRevision === authority.sourceRevision,
  );
}

export async function requestPinPreview(
  graphPath: string,
  pinId: string,
): Promise<PinPreviewRequestResult> {
  const captured = capturePreviewRequest(graphPath, pinId);
  if (typeof captured === 'string') return reject(captured);

  try {
    assertCurrentProjectIdentity(captured.authority.project);
  } catch {
    return reject('stale-project-lifecycle');
  }
  if (!isPreviewAuthorityCurrent(graphPath, captured.authority)) {
    return reject('missing-resource');
  }

  const store = useExecutionStore.getState();
  const generation = store.beginPinPreview(graphPath, captured.output.port);
  const outcome: GraphRunOutcomeState = { outcome: 'success' };
  const observation: PinPreviewObservation = {
    projectSessionId: null,
    output: captured.output,
    generation,
    runId: null,
    terminal: 'pending',
  };
  const rejectStaleSettlement = (): PinPreviewRequestResult => {
    store.removePinPreview(graphPath, captured.output.port, generation);
    return { status: 'rejected', reason: 'stale-project-lifecycle' };
  };

  try {
    await ProjectService.executeGraphDocument(
      captured.authority.project.projectInstanceId,
      graphPath,
      {
        type: 'outputs',
        outputs: [captured.output],
        includeDefaultResults: false,
      },
      (event) => {
        if (!isPreviewAuthorityCurrent(graphPath, captured.authority)) return;
        observeGraphRunEvent(graphPath, event, outcome, observation);
      },
    );
  } catch (error) {
    if (!isPreviewAuthorityCurrent(graphPath, captured.authority)) {
      return rejectStaleSettlement();
    }
    const message = formatErrorMessage(error);
    store.failPinPreview(graphPath, captured.output.port, generation, message);
    uiStore.showToast(`预览失败：${message}`, 'error', 4000);
    return { status: 'failed', generation, error: message };
  }

  if (!isPreviewAuthorityCurrent(graphPath, captured.authority)) {
    return rejectStaleSettlement();
  }
  const preview = lookupPinPreview(
    useExecutionStore.getState().getGraph(graphPath).pinPreviews,
    graphPath,
    captured.output.port,
  );
  if (
    observation.terminal !== 'completed'
    || preview?.generation !== generation
    || preview.status !== 'ready'
    || !preview.sourceId
  ) {
    const message = observation.terminal === 'cancelled'
      ? 'preview run was cancelled'
      : 'preview output was not published';
    store.failPinPreview(graphPath, captured.output.port, generation, message);
    return { status: 'failed', generation, error: message };
  }

  return {
    status: 'completed',
    generation,
    sourceId: preview.sourceId,
  };
}

export async function requestAndOpenPinPreview(
  graphPath: string,
  pinId: string,
  t: TFunction,
): Promise<boolean> {
  const result = await requestPinPreview(graphPath, pinId);
  if (result.status !== 'completed') return false;
  return openInspectableSource(windowSourceRef(result.sourceId), t);
}
