import type { TFunction } from 'i18next';
import {
  resolveInspectableSource,
  windowSourceRef,
  type InspectableSourceRef,
  type Presentation,
  type SourceDescriptor,
} from '@/features/core/resultSource';
import {
  openPresentationWindow,
  presentationWindowPayload,
  presentationWindowPayloadFromDescriptor,
} from '@/features/application/window';
import { uiStore } from '@/features/core/ui/UIStore';
import {
  evaluatePinViewState,
  pinViewDisabledTitle,
  type ResolvePinViewTargetParams,
} from '@/features/core/execution/pinViewTarget';

export async function launchInspectablePresentation(
  descriptor: SourceDescriptor,
  titleFallback: string,
): Promise<void> {
  await openPresentationWindow(
    descriptor.sourceId,
    presentationWindowPayloadFromDescriptor(descriptor, titleFallback),
  );
}

export async function openInspectableSource(
  ref: InspectableSourceRef,
  t: TFunction,
  options?: { silent?: boolean },
): Promise<boolean> {
  try {
    const descriptor = await resolveInspectableSource(ref);
    if (!descriptor) {
      if (!options?.silent) {
        uiStore.showToast(t('sourceInspector.noSource'), 'error');
      }
      return false;
    }
    await launchInspectablePresentation(descriptor, t('contextMenu.pin.view'));
    return true;
  } catch (error) {
    if (!options?.silent) {
      const message = error instanceof Error ? error.message : String(error);
      uiStore.showToast(t('toast.viewOpenFailed', { error: message }), 'error');
    }
    return false;
  }
}

/** Open pin/context-menu targets; tries upstream pins in order for input direction. */
export async function openPinInspectableView(
  params: ResolvePinViewTargetParams,
  t: TFunction,
): Promise<boolean> {
  const { refs, disabledReason } = evaluatePinViewState(params);
  for (const ref of refs) {
    if (await openInspectableSource(ref, t, { silent: true })) {
      return true;
    }
  }

  const hint = pinViewDisabledTitle(disabledReason, t);
  uiStore.showToast(hint ?? t('sourceInspector.noSource'), 'error');
  return false;
}

/** Execution `openSourceWindow` events — window-owned sources via InspectableSourceRef. */
export async function openWindowInspectableSource(
  sourceId: string,
  event: { presentation: Presentation; windowTitle: string },
): Promise<void> {
  const descriptor = await resolveInspectableSource(windowSourceRef(sourceId));
  if (descriptor) {
    await launchInspectablePresentation(descriptor, event.windowTitle);
    return;
  }

  await openPresentationWindow(
    sourceId,
    presentationWindowPayload(event.presentation, event.windowTitle),
  );
}
