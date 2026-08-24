import type { TFunction } from 'i18next';
import { showWorkbenchLayoutError } from '@/features/application/layout/workbenchLayoutErrorFeedback';
import {
  openPresentationWindow,
  presentationWindowPayloadFromDescriptor,
} from '@/features/application/window';
import { workbenchDockviewPort } from '@/features/core/dockview/workbenchDockviewPort';
import {
  evaluatePinViewState,
  type ResolvePinViewTargetParams,
} from '@/features/core/execution/pinViewTarget';
import { useExecutionStore } from '@/features/core/execution/useExecutionStore';
import {
  captureProjectIdentity,
  isCurrentProjectIdentity,
} from '@/features/core/projectLifecycle/projectLifecycleAuthority';
import {
  resolveInspectableResult,
  resolveInspectableResultRef,
  type InspectableResultRef,
  type ResultDescriptor,
} from '@/features/core/resultSource';
import { resultPanelKey } from '@/features/domain/result';

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
  _t: TFunction,
  options?: { selectedResultId?: string | null },
): Promise<boolean> {
  let descriptor: ResultDescriptor | null;
  try {
    const project = captureProjectIdentity();
    const resolved = await resolveInspectableResultRef(ref, options?.selectedResultId);
    if (!isCurrentProjectIdentity(project)) return false;
    if (resolved.history) useExecutionStore.getState().recordPinHistory(resolved.history);
    descriptor = resolved.ref ? await resolveInspectableResult(resolved.ref) : null;
    if (!isCurrentProjectIdentity(project) || !descriptor) return false;
  } catch {
    return false;
  }

  try {
    await workbenchDockviewPort.upsertResult({
      resultKey: resultPanelKey(descriptor),
      resultId: descriptor.resultId,
      title: descriptor.title,
      presentation: descriptor.presentation,
      source: descriptor.provenance.output,
    });
    return true;
  } catch (error) {
    showWorkbenchLayoutError(error);
    return false;
  }
}

/** Open pin/context-menu targets; tries upstream pins in order for input direction. */
export async function openPinInspectableView(
  params: ResolvePinViewTargetParams,
  t: TFunction,
  options?: { selectedResultId?: string | null },
): Promise<boolean> {
  const { refs } = evaluatePinViewState(params);
  for (const ref of refs) {
    if (await openInspectableResult(ref, t, {
      selectedResultId: options?.selectedResultId,
    })) {
      return true;
    }
  }

  return false;
}
