import { createPersistedWindow } from "./createPersistedWindow";
import { windowKindForRoute } from "./windowRoute";
import { logger } from "@/features/application/observability/appLogger";
import { normalizeApplicationIpcError } from "@/features/application/errorReference";
import { resultLeases } from "@/features/application/results/resultLeases";
import type { ResultReference } from "@/shared/types/domain/result";
import {
  plotTypeFromPresentation,
  presentationRoute,
  type Presentation,
} from "@/features/application/results";

export interface PresentationWindowPayload {
  route: string;
  windowTitle: string;
  plotType?: string;
}

export function presentationWindowPayload(
  presentation: Presentation,
  windowTitle: string,
): PresentationWindowPayload {
  return {
    route: presentationRoute(presentation),
    windowTitle,
    plotType: plotTypeFromPresentation(presentation),
  };
}

export function presentationWindowPayloadFromDescriptor(
  descriptor: { presentation: Presentation; title: string },
  titleFallback: string,
): PresentationWindowPayload {
  return presentationWindowPayload(descriptor.presentation, descriptor.title || titleFallback);
}

export async function openPresentationWindow(
  reference: ResultReference,
  presentation: PresentationWindowPayload,
): Promise<void> {
  const route = presentation.route || "/info";
  const labelKind = route.replace(/^\//, "") || "source";
  const label = `${labelKind}-${crypto.randomUUID()}`;
  const held = await resultLeases.acquire(reference, label);
  const params = new URLSearchParams({
    resultId: reference.resultId,
    executionSessionId: reference.executionSessionId,
    leaseId: held.leaseId,
  });
  if (presentation.plotType) params.set("plotType", presentation.plotType);
  const url = `index.html#${route}?${params.toString()}`;

  let created = false;
  try {
    await createPersistedWindow({
      geometry: { source: "backend", kind: windowKindForRoute(route) },
      label,
      url,
      title: presentation.windowTitle.trim() || "Source Inspector",
    });
    created = true;
  } catch (error) {
    const ipcError = normalizeApplicationIpcError("open_presentation_window", error);
    logger.exec.error(
      `Failed to open presentation window code=${ipcError.code} incidentId=${ipcError.incidentId ?? "none"}`,
      "Window",
    );
    throw error;
  } finally {
    await resultLeases.finish(held.leaseId, created);
  }
}
