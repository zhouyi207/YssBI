import { showWorkbenchLayoutError } from "@/modules/workbench/public";
import {
  openPresentationWindow,
  presentationWindowPayloadFromDescriptor,
} from "@/features/application/window";
import { workbenchLayoutControl } from "@/modules/workbench/public";
import {
  inspectableRefsFromPinView,
  type ResolvePinViewTargetParams,
} from "@/features/core/execution/pinViewTarget";
import {
  captureProjectIdentity,
  isCurrentProjectIdentity,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import {
  resolveInspectableResultRef,
  type InspectableResultRef,
} from "@/features/application/results";
import {
  resultReference,
  type ResultDescriptor,
  type ResultReference,
} from "@/shared/types/domain/result";
import { isApplicationIpcErrorCode } from "@/features/application/errorReference";
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

export async function openInspectableResult(ref: InspectableResultRef): Promise<boolean> {
  let project: ReturnType<typeof captureProjectIdentity>;
  let reference: ResultReference | null;
  try {
    project = captureProjectIdentity();
    const dependencies = {
      coordinator: resultQueryCoordinator,
      read: resultQueryRead,
    };
    reference = await resolveInspectableResultRef(ref, dependencies);
    if (!isCurrentProjectIdentity(project) || !reference) return false;
  } catch {
    return false;
  }

  let leaseId: string | undefined;
  let installed = false;
  try {
    const held = await resultLeases.acquire(reference);
    leaseId = held.leaseId;
    if (!isCurrentProjectIdentity(project)) return false;
    const panel = await workbenchLayoutControl.upsertResult({
      reference: resultReference(held.descriptor),
      leaseId,
      title: held.descriptor.title,
      presentation: held.descriptor.presentation,
    });
    installed = panel.metadata.role === "result" && panel.metadata.leaseId === leaseId;
    return true;
  } catch (error) {
    if (
      !isCurrentProjectIdentity(project) ||
      isApplicationIpcErrorCode(error, "result_not_found") ||
      isApplicationIpcErrorCode(error, "stale_result_reference")
    )
      return false;
    showWorkbenchLayoutError(error);
    return false;
  } finally {
    if (leaseId) await resultLeases.finish(leaseId, installed);
  }
}

/** Open pin/context-menu targets; tries upstream pins in order for input direction. */
export async function openPinInspectableView(params: ResolvePinViewTargetParams): Promise<boolean> {
  const refs = inspectableRefsFromPinView(params);
  for (const ref of refs) {
    if (await openInspectableResult(ref)) {
      return true;
    }
  }

  return false;
}
