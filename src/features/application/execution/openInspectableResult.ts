import type { TFunction } from 'i18next';
import {
  resolveInspectableResult,
  resolveInspectableResultRef,
  resultRef,
  type InspectableResultRef,
  type Presentation,
  type ResultDescriptor,
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
import { useExecutionStore } from '@/features/core/execution/useExecutionStore';

export async function launchInspectablePresentation(
  descriptor: ResultDescriptor,
  titleFallback: string,
): Promise<void> {
  await openPresentationWindow(
    descriptor.resultId,
    presentationWindowPayloadFromDescriptor(descriptor, titleFallback),
  );
}

export async function openInspectableResult(
  ref: InspectableResultRef,
  t: TFunction,
  options?: { silent?: boolean; selectedResultId?: string | null },
): Promise<boolean> {
  try {
    const resolved = await resolveInspectableResultRef(ref, options?.selectedResultId);
    if (resolved.history) {
      useExecutionStore.getState().recordPinHistory(resolved.history);
    }
    const descriptor = resolved.ref
      ? await resolveInspectableResult(resolved.ref)
      : null;
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
  options?: { selectedResultId?: string | null },
): Promise<boolean> {
  const { refs, disabledReason } = evaluatePinViewState(params);
  for (const ref of refs) {
    if (await openInspectableResult(ref, t, {
      silent: true,
      selectedResultId: options?.selectedResultId,
    })) {
      return true;
    }
  }

  const hint = pinViewDisabledTitle(disabledReason, t);
  uiStore.showToast(hint ?? t('sourceInspector.noSource'), 'error');
  return false;
}

/** Open an exact result requested by an execution window event. */
export async function openWindowInspectableResult(
  resultId: string,
  event: { presentation: Presentation; windowTitle: string },
): Promise<void> {
  const descriptor = await resolveInspectableResult(resultRef(resultId));
  if (descriptor) {
    await launchInspectablePresentation(descriptor, event.windowTitle);
    return;
  }

  await openPresentationWindow(
    resultId,
    presentationWindowPayload(event.presentation, event.windowTitle),
  );
}
