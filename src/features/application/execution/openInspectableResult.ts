import type { TFunction } from 'i18next';
import {
  resolveInspectableResult,
  resolveInspectableResultRef,
  type InspectableResultRef,
  type ResultDescriptor,
} from '@/features/core/resultSource';
import {
  openPresentationWindow,
  presentationWindowPayloadFromDescriptor,
} from '@/features/application/window';
import {
  evaluatePinViewState,
  type ResolvePinViewTargetParams,
} from '@/features/core/execution/pinViewTarget';
import { focusResultSidebar } from '@/features/application/editor/rightSidebarActions';
import { useExecutionStore } from '@/features/core/execution/useExecutionStore';
import { useResultWorkspaceStore } from '@/features/core/resultWorkspace';
import {
  captureProjectIdentity,
  isCurrentProjectIdentity,
} from '@/features/core/projectLifecycle/projectLifecycleAuthority';

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
  try {
    const project = captureProjectIdentity();
    const resolved = await resolveInspectableResultRef(ref, options?.selectedResultId);
    if (!isCurrentProjectIdentity(project)) return false;
    if (resolved.history) useExecutionStore.getState().recordPinHistory(resolved.history);
    const descriptor = resolved.ref ? await resolveInspectableResult(resolved.ref) : null;
    if (!isCurrentProjectIdentity(project) || !descriptor) return false;
    useResultWorkspaceStore.getState().openResult(descriptor);
    focusResultSidebar();
    return true;
  } catch {
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
