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
import { resultReference, type ResultDescriptor } from "@/shared/types/domain/result";
import { resultLeases } from "@/features/application/results/resultLeases";
import { resultQueryCoordinator, resultQueryRead } from "@/features/application/results";

export async function launchInspectablePresentation(
  descriptor: ResultDescriptor,
  titleFallback: string,
): Promise<void> {
  await openPresentationWindow(
    resultReference(descriptor),
    presentationWindowPayloadFromDescriptor(descriptor, titleFallback),
  );
}

export async function openInspectableResult(
  ref: InspectableResultRef,
  _t: TFunction,
): Promise<boolean> {
  let project: ReturnType<typeof captureProjectIdentity>;
  let descriptor: ResultDescriptor | null;
  try {
    project = captureProjectIdentity();
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

  let leaseId: string | undefined;
  let installed = false;
  try {
    const held = await resultLeases.acquire(descriptor);
    leaseId = held.leaseId;
    if (!isCurrentProjectIdentity(project)) return false;
    const panel = await workbenchDockviewControl.upsertResult({
      reference: resultReference(held.descriptor),
      leaseId,
      title: held.descriptor.title,
      presentation: held.descriptor.presentation,
    });
    installed = panel.metadata.role === "result" && panel.metadata.leaseId === leaseId;
    return true;
  } catch (error) {
    showWorkbenchLayoutError(error);
    return false;
  } finally {
    if (leaseId) await resultLeases.finish(leaseId, installed);
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
