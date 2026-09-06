import type { TFunction } from "i18next";
import { showWorkbenchLayoutError } from "@/modules/workbench/public";
import {
  openPresentationWindow,
  presentationWindowPayloadFromDescriptor,
} from "@/features/application/window";
import { workbenchDockviewControl } from "@/modules/workbench/public";
import {
  evaluatePinViewState,
  type ResolvePinViewTargetParams,
} from "@/features/core/execution/pinViewTarget";
import {
  captureProjectIdentity,
  isCurrentProjectIdentity,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import {
  resolveInspectableResult,
  resolveInspectableResultRef,
  type InspectableResultRef,
} from "@/features/application/results";
import type { ResultDescriptor } from "@/shared/types/domain/result";
import { resultQueryCoordinator, resultQueryRead } from "@/features/application/results";
import { resultPanelKey } from "@/features/domain/result";

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
): Promise<boolean> {
  let descriptor: ResultDescriptor | null;
  try {
    const project = captureProjectIdentity();
    const dependencies = {
      coordinator: resultQueryCoordinator,
      read: resultQueryRead,
    };
    const resolved = await resolveInspectableResultRef(ref, dependencies);
    if (!isCurrentProjectIdentity(project)) return false;
    descriptor = resolved.ref
      ? (structuredClone(
          await resolveInspectableResult(resolved.ref, dependencies),
        ) as ResultDescriptor | null)
      : null;
    if (!isCurrentProjectIdentity(project) || !descriptor) return false;
  } catch {
    return false;
  }

  try {
    await workbenchDockviewControl.upsertResult({
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
): Promise<boolean> {
  const { refs } = evaluatePinViewState(params);
  for (const ref of refs) {
    if (await openInspectableResult(ref, t)) {
      return true;
    }
  }

  return false;
}
