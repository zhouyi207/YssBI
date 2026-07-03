import { toast } from 'sonner';
import type { TFunction } from 'i18next';
import { otherEndpointFromConnectionId } from '@/features/core/dataStore/pinLinks';
import { SourceService, plotTypeFromPresentation, presentationRoute } from '@/features/core/dataView';
import { openPresentationWindow } from '@/features/application/window';
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
  pinResults?: Map<string, PinResultState>;
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

function pinResultFromDescriptor(
  graphId: string,
  pinId: string,
  descriptor: NonNullable<Awaited<ReturnType<typeof SourceService.getPinDescriptor>>>,
): PinResultState {
  return {
    graphId,
    nodeId: '',
    pinId,
    sourceId: descriptor.sourceId,
    descriptor,
  };
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

export async function resolvePinViewTarget(
  params: ResolvePinViewTargetParams,
): Promise<PinViewTarget | null> {
  const cached = resolvePinViewTargetFromCache(params);
  if (cached) return cached;

  const { graphId, pinId, direction, pinType, connectionIds } = params;
  if (!isInspectableDataPin(pinType)) return null;

  let descriptorPinIds: string[] = [];
  if (direction === 'output') {
    descriptorPinIds = [pinId];
  } else {
    descriptorPinIds = resolveUpstreamPinIds(pinId, connectionIds);
    if (descriptorPinIds.length === 0) return null;
  }

  for (const descriptorPinId of descriptorPinIds) {
    const descriptor = await SourceService.getPinDescriptor(graphId, descriptorPinId);
    if (descriptor) {
      return {
        sourcePinId: descriptorPinId,
        pinResult: pinResultFromDescriptor(graphId, descriptorPinId, descriptor),
      };
    }
  }
  return null;
}

export async function openPinView(
  params: ResolvePinViewTargetParams,
  t: TFunction,
): Promise<boolean> {
  try {
    const target = await resolvePinViewTarget(params);
    if (!target) {
      const reason = resolvePinViewDisabledReason(params);
      const hint = pinViewDisabledTitle(reason, t);
      if (hint) toast.error(hint);
      return false;
    }

    await openPresentationWindow(target.pinResult.sourceId, {
      route: presentationRoute(target.pinResult.descriptor.presentation),
      windowTitle: target.pinResult.descriptor.title || t('contextMenu.pin.view'),
      plotType: plotTypeFromPresentation(target.pinResult.descriptor.presentation),
    });
    return true;
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    toast.error(t('toast.viewOpenFailed', { error: message }));
    return false;
  }
}

export function buildPinViewParams(input: {
  graphId: string;
  pinId: string;
  direction: 'input' | 'output';
  pinType: string;
  connectionIds?: readonly string[];
  pinResults?: Map<string, PinResultState>;
  executionStatus?: ExecutionStatus;
}): ResolvePinViewTargetParams {
  return input;
}
