import { createPersistedWindow } from './createPersistedWindow';
import { windowKindForRoute } from './windowRoute';
import { logger } from '@/features/application/observability/appLogger';
import { normalizeApplicationIpcError } from '@/features/application/errorReference';
import {
  plotTypeFromPresentation,
  presentationRoute,
  type Presentation,
} from '@/features/application/results';

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
  return presentationWindowPayload(
    descriptor.presentation,
    descriptor.title || titleFallback,
  );
}

export async function openPresentationWindow(
  resultId: string,
  presentation: PresentationWindowPayload,
): Promise<void> {
  const route = presentation.route || '/info';
  const labelKind = route.replace(/^\//, '') || 'source';
  const label = `${labelKind}-${Math.random().toString(36).substring(2, 10)}`;
  const params = new URLSearchParams({ resultId });
  if (presentation.plotType) params.set('plotType', presentation.plotType);
  const url = `index.html#${route}?${params.toString()}`;

  try {
    await createPersistedWindow({
      geometry: { source: 'backend', kind: windowKindForRoute(route) },
      label,
      url,
      title: presentation.windowTitle.trim() || 'Source Inspector',
    });
  } catch (error) {
    const ipcError = normalizeApplicationIpcError('open_presentation_window', error);
    logger.exec.error(
      `Failed to open presentation window code=${ipcError.code} incidentId=${ipcError.incidentId ?? 'none'}`,
      'Window',
    );
    throw error;
  }
}
