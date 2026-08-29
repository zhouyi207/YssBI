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
import { ProjectService } from '@/services/project/projectService';
import { PinPreviewGenerationService } from '@/services/nodeSystem/pinPreviewGenerationService';
import { normalizeApplicationIpcError } from '@/features/application/errorReference';
import { openInspectableResult } from '@/features/application/execution/openInspectableResult';
import { resultRef } from '@/features/application/results';
import type { PortAddressDto } from '@/shared/types/domain/editorProjection';
import type { GraphOutputRefDto } from '@/shared/types/domain/executionDemand';
import type { PinData } from '@/shared/types/store/graph';
import { inferGraphResourceKind } from '@/shared/types/domain/graphResourcePath';

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
  | 'generation-exhausted'
  | 'stale-project-lifecycle';

export interface PinPreviewFailure {
  code: string;
  incidentId: string | null;
}

export type PinPreviewRequestResult =
  | { status: 'completed'; generation: number; resultId: string }
  | { status: 'rejected'; reason: PinPreviewRejectionReason }
  | { status: 'failed'; generation: number; error: PinPreviewFailure };

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
  return { status: 'rejected', reason };
}

function staleSettlement(): PinPreviewRequestResult {
  return { status: 'rejected', reason: 'stale-project-lifecycle' };
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
  if (captured === 'stale-project-lifecycle') return staleSettlement();
  if (typeof captured === 'string') return reject(captured);

  try {
    assertCurrentProjectIdentity(captured.authority.project);
  } catch {
    return staleSettlement();
  }
  if (!isPreviewAuthorityCurrent(graphPath, captured.authority)) {
    return staleSettlement();
  }

  let generation: number;
  try {
    generation = await PinPreviewGenerationService.allocate();
  } catch {
    return { status: 'rejected', reason: 'generation-exhausted' };
  }
  if (
    !isCurrentProjectIdentity(captured.authority.project)
    || !isPreviewAuthorityCurrent(graphPath, captured.authority)
  ) return staleSettlement();

  const store = useExecutionStore.getState();
  const lease = store.beginPinPreview(graphPath, captured.output.port, generation);
  const outcome: GraphRunOutcomeState = { outcome: 'success' };
  const observation: PinPreviewObservation = {
    projectSessionId: null,
    output: captured.output,
    generation,
    runId: null,
    terminal: 'pending',
    stale: false,
    lease,
  };

  try {
    await ProjectService.executeGraphDocument(
      captured.authority.project.projectInstanceId,
      graphPath,
      {
        type: 'pinPreview',
        output: captured.output,
        generation,
      },
      (event) => {
        if (!lease.isCurrent()) return;
        if (!isPreviewAuthorityCurrent(graphPath, captured.authority)) return;
        observeGraphRunEvent(graphPath, event, outcome, observation);
      },
    );
  } catch (error) {
    if (!lease.isCurrent()) return staleSettlement();
    if (
      !isCurrentProjectIdentity(captured.authority.project)
      || !isPreviewAuthorityCurrent(graphPath, captured.authority)
      || observation.stale
    ) {
      return staleSettlement();
    }
    const current = lookupPinPreview(
      useExecutionStore.getState().getGraph(graphPath).pinPreviews,
      graphPath,
      captured.output.port,
    );
    if (current?.generation !== generation) return staleSettlement();
    const ipcError = normalizeApplicationIpcError('execute_graph_document', error);
    const failure = { code: ipcError.code, incidentId: ipcError.incidentId };
    lease.fail(failure.code);
    return { status: 'failed', generation, error: failure };
  }

  if (!lease.isCurrent()) return staleSettlement();
  if (!isCurrentProjectIdentity(captured.authority.project)) {
    return staleSettlement();
  }
  if (!isPreviewAuthorityCurrent(graphPath, captured.authority) || observation.stale) {
    return staleSettlement();
  }
  const preview = lookupPinPreview(
    useExecutionStore.getState().getGraph(graphPath).pinPreviews,
    graphPath,
    captured.output.port,
  );
  if (preview?.generation !== generation) return staleSettlement();
  if (
    observation.terminal === 'completed'
    && preview.status === 'ready'
    && preview.resultId
  ) {
    return {
      status: 'completed',
      generation,
      resultId: preview.resultId,
    };
  }
  if (observation.terminal === 'pending' || observation.terminal === 'completed') {
    return staleSettlement();
  }
  const failure: PinPreviewFailure = {
    code: observation.terminal === 'cancelled' ? 'run_cancelled' : 'run_failed',
    incidentId: null,
  };
  lease.fail(failure.code);
  return { status: 'failed', generation, error: failure };
}

export async function requestAndOpenPinPreview(
  graphPath: string,
  pinId: string,
  t: TFunction,
): Promise<boolean> {
  const result = await requestPinPreview(graphPath, pinId);
  if (result.status !== 'completed') return false;
  return openInspectableResult(resultRef(result.resultId), t);
}
