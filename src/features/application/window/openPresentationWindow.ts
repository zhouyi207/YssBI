import { createPersistedWindow } from './createPersistedWindow';
import { windowKindForRoute } from './windowRoute';
import { logger } from '@/utils/appLogger';
import {
  plotTypeFromPresentation,
  presentationRoute,
  type Presentation,
} from '@/features/core/resultSource';

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
  sourceId: string,
  presentation: PresentationWindowPayload,
): Promise<void> {
  const route = presentation.route || '/info';
  const labelKind = route.replace(/^\//, '') || 'source';
  const label = `${labelKind}-${Math.random().toString(36).substring(2, 10)}`;
  const params = new URLSearchParams({ sourceId });
  if (presentation.plotType) params.set('plotType', presentation.plotType);
  const url = `index.html#${route}?${params.toString()}`;

  await createPersistedWindow({
    kind: windowKindForRoute(route),
    label,
    url,
    title: presentation.windowTitle.trim() || 'Source Inspector',
  });
}

export async function openPresentationWindowSafe(
  sourceId: string,
  presentation: PresentationWindowPayload,
  logTag = 'Window',
): Promise<void> {
  try {
    await openPresentationWindow(sourceId, presentation);
  } catch (e) {
    logger.exec.error(
      `Failed to open window: ${e instanceof Error ? e.message : String(e)}`,
      logTag,
    );
  }
}
