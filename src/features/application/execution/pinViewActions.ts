import type { TFunction } from 'i18next';
import { SourceService } from '@/services/resultSource/resultSourceService';
import { plotTypeFromPresentation, presentationRoute } from '@/features/core/resultSource';
import { openPresentationWindow } from '@/features/application/window';
import { uiStore } from '@/features/core/ui/UIStore';
import type { PinResultState } from '@/shared/types/ui';
import type { SourceDescriptor } from '@/features/core/resultSource/types';
import {
  isInspectableDataPin,
  pinViewDisabledTitle,
  resolvePinViewDisabledReason,
  resolvePinViewTargetFromCache,
  resolveUpstreamPinIds,
  type PinViewTarget,
  type ResolvePinViewTargetParams,
} from '@/features/core/execution/pinViewTarget';

function pinResultFromDescriptor(
  graphId: string,
  pinId: string,
  descriptor: SourceDescriptor,
): PinResultState {
  return {
    graphId,
    nodeId: '',
    pinId,
    sourceId: descriptor.sourceId,
    descriptor,
  };
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
      if (hint) uiStore.showToast(hint, 'error');
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
    uiStore.showToast(t('toast.viewOpenFailed', { error: message }), 'error');
    return false;
  }
}
