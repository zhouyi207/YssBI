import type { TFunction } from 'i18next';
import { otherEndpointFromConnectionId } from '@/features/core/dataStore/pinLinks';
import type { InspectableSourceRef } from '@/features/core/resultSource/inspectableSource';
import { runtimePinRef } from '@/features/core/resultSource/inspectableSource';
import type { ExecutionStatus, PinResultState } from '@/shared/types/ui';
import { lookupPinResult } from './pinResultIndex';

export type PinViewDisabledReason =
  | 'exec_pin'
  | 'not_applicable'
  | 'no_run'
  | 'no_upstream';

export interface ResolvePinViewTargetParams {
  graphPath: string;
  pinId: string;
  direction: 'input' | 'output';
  isExec: boolean;
  connectionIds?: readonly string[];
  pinResults?: ReadonlyMap<string, PinResultState>;
  executionStatus?: ExecutionStatus;
}

export interface PinViewUiState {
  showMenu: boolean;
  enabled: boolean;
  disabledReason: PinViewDisabledReason | null;
  refs: InspectableSourceRef[];
}

function isInspectableDataPin(isExec: boolean): boolean {
  return !isExec;
}

export function resolveUpstreamPinIds(
  pinId: string,
  connectionIds: readonly string[] | undefined,
): string[] {
  return (connectionIds ?? []).map((connectionId) =>
    otherEndpointFromConnectionId(connectionId, pinId),
  );
}

/** Shared traversal: output pin or upstream data pins for input direction. */
export function candidatePinIdsFromPinView(
  params: ResolvePinViewTargetParams,
): string[] {
  const { pinId, direction, isExec, connectionIds } = params;
  if (!isInspectableDataPin(isExec)) return [];
  return direction === 'output' ? [pinId] : resolveUpstreamPinIds(pinId, connectionIds);
}

function buildInspectableRefs(
  params: ResolvePinViewTargetParams,
  candidates: string[],
): InspectableSourceRef[] {
  const { graphPath, pinResults } = params;
  return candidates.map((candidatePinId) => {
    const cached = lookupPinResult(pinResults, graphPath, candidatePinId);
    return runtimePinRef(cached?.graphPath ?? graphPath, cached?.pinId ?? candidatePinId);
  });
}

/** Single-pass UI + open resolution for pin view menus. */
export function evaluatePinViewState(params: ResolvePinViewTargetParams): PinViewUiState {
  const { isExec, executionStatus, direction } = params;

  if (!isInspectableDataPin(isExec)) {
    return {
      showMenu: false,
      enabled: false,
      disabledReason: 'exec_pin',
      refs: [],
    };
  }

  const candidates = candidatePinIdsFromPinView(params);
  const refs = buildInspectableRefs(params, candidates);
  const cacheHit = candidates.some((candidatePinId) =>
    lookupPinResult(params.pinResults, params.graphPath, candidatePinId),
  );

  if (cacheHit) {
    return {
      showMenu: true,
      enabled: true,
      disabledReason: null,
      refs,
    };
  }

  if (direction === 'input' && candidates.length === 0) {
    return {
      showMenu: false,
      enabled: false,
      disabledReason: 'not_applicable',
      refs,
    };
  }

  const enabled = executionStatus === 'completed' && refs.length > 0;
  return {
    showMenu: true,
    enabled,
    disabledReason: enabled
      ? null
      : direction === 'input'
        ? 'no_upstream'
        : 'no_run',
    refs,
  };
}

/** Build backend lookup refs for opening a pin view (no IPC). */
export function inspectableRefsFromPinView(
  params: ResolvePinViewTargetParams,
): InspectableSourceRef[] {
  return evaluatePinViewState(params).refs;
}

export function pinViewDisabledTitle(
  reason: PinViewDisabledReason | null,
  t: TFunction,
): string | undefined {
  if (!reason || reason === 'exec_pin' || reason === 'not_applicable') return undefined;
  const key = {
    no_run: 'contextMenu.pin.viewDisabledNoRun',
    no_upstream: 'contextMenu.pin.viewDisabledNoUpstream',
  }[reason];
  return t(key);
}

export function buildPinViewParams(input: {
  graphPath: string;
  pinId: string;
  direction: 'input' | 'output';
  isExec: boolean;
  connectionIds?: readonly string[];
  pinResults?: ReadonlyMap<string, PinResultState>;
  executionStatus?: ExecutionStatus;
}): ResolvePinViewTargetParams {
  return input;
}
