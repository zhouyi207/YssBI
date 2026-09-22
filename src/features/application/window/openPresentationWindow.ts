import { createPersistedWindow } from "./createPersistedWindow";
import { logger } from "@/features/application/observability/appLogger";
import { normalizeApplicationIpcError } from "@/features/application/errorReference";
import { resultLeases } from "@/features/application/results/resultLeases";
import type { ResultReference } from "@/shared/types/domain/result";
import type { ResultPresentation } from "@/shared/types/domain/result";

const presentationWindowKinds = {
  inspector: "inspect",
  plot: "plot",
  report: "info",
} as const satisfies Record<ResultPresentation["kind"], PresentationWindowPayload["kind"]>;

export interface PresentationWindowPayload {
  kind: "inspect" | "plot" | "info";
  windowTitle: string;
}

export function presentationWindowPayloadFromDescriptor(
  descriptor: { presentation: ResultPresentation; title: string },
  titleFallback: string,
): PresentationWindowPayload {
  return {
    kind: presentationWindowKinds[descriptor.presentation.kind],
    windowTitle: descriptor.title || titleFallback,
  };
}

export async function openPresentationWindow(
  reference: ResultReference,
  presentation: PresentationWindowPayload,
): Promise<void> {
  const kind = presentation.kind;
  const route = `/${kind}`;
  const label = `${kind}-${crypto.randomUUID()}`;
  const held = await resultLeases.acquire(reference, label);
  const params = new URLSearchParams({
    resultId: reference.resultId,
    executionSessionId: reference.executionSessionId,
    leaseId: held.leaseId,
  });
  const url = `index.html#${route}?${params.toString()}`;

  let created = false;
  try {
    await createPersistedWindow({
      kind,
      label,
      url,
      title: presentation.windowTitle.trim() || "Source Inspector",
    });
    created = true;
  } catch (error) {
    const ipcError = normalizeApplicationIpcError(error);
    logger.exec.error(
      `Failed to open presentation window code=${ipcError.code} incidentId=${ipcError.incidentId ?? "none"}`,
      "Window",
    );
    throw error;
  } finally {
    await resultLeases.finish(held.leaseId, created);
  }
}
