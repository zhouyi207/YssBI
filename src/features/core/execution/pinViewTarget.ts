import type { TFunction } from 'i18next';
import { otherEndpointFromConnectionId } from '@/features/core/dataStore/pinLinks';
import type { ExecutionStatus, PinResultState } from '@/shared/types/ui';

export type PinViewDisabledReason =
  | 'exec_pin'
  | 'not_applicable'
  | 'no_run'
  | 'no_upstream';

export interface PinViewTarget {
  sourcePinId: string;
  pinResult: PinResultState;
}

export interface ResolvePinViewTargetParams {
  graphId: string;
  pinId: string;
  direction: 'input' | 'output';
  pinType: string;
  connectionIds?: readonly string[];
  pinResults?: ReadonlyMap<string, PinResultState>;
  executionStatus?: ExecutionStatus;
}

export function isInspectableDataPin(pinType: string): boolean {
  return pinType !== 'exec';
}

export function resolveUpstreamPinIds(
  pinId: string,
  connectionIds: readonly string[] | undefined,
): string[] {
  return (connectionIds ?? []).map((connectionId) =>
    otherEndpointFromConnectionId(connectionId, pinId),
  );
}

export function resolvePinViewTargetFromCache(
  params: ResolvePinViewTargetParams,
): PinViewTarget | null {
  const { graphId, pinId, direction, pinType, connectionIds, pinResults } = params;
  if (!isInspectableDataPin(pinType) || !pinResults) return null;

  if (direction === 'output') {
    const pinResult = pinResults.get(pinId);
    if (pinResult?.graphId === graphId) {
      return { sourcePinId: pinId, pinResult };
    }
    return null;
  }

  for (const upstreamPinId of resolveUpstreamPinIds(pinId, connectionIds)) {
    const pinResult = pinResults.get(upstreamPinId);
    if (pinResult?.graphId === graphId) {
      return { sourcePinId: upstreamPinId, pinResult };
    }
  }
  return null;
}

export function resolvePinViewDisabledReason(
  params: ResolvePinViewTargetParams,
): PinViewDisabledReason | null {
  const { pinType, direction, connectionIds } = params;
  if (!isInspectableDataPin(pinType)) return 'exec_pin';
  if (resolvePinViewTargetFromCache(params)) return null;

  if (direction === 'input') {
    if (resolveUpstreamPinIds(params.pinId, connectionIds).length === 0) {
      return 'not_applicable';
    }
    return 'no_upstream';
  }

  return 'no_run';
}

export function shouldShowPinViewMenuItem(params: ResolvePinViewTargetParams): boolean {
  const reason = resolvePinViewDisabledReason(params);
  return reason !== 'exec_pin' && reason !== 'not_applicable';
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
  graphId: string;
  pinId: string;
  direction: 'input' | 'output';
  pinType: string;
  connectionIds?: readonly string[];
  pinResults?: ReadonlyMap<string, PinResultState>;
  executionStatus?: ExecutionStatus;
}): ResolvePinViewTargetParams {
  return input;
}
