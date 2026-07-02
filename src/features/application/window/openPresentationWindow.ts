import { createPersistedWindow } from './createPersistedWindow';
import { windowKindForRoute } from './windowRoute';
import { logger } from '@/utils/appLogger';

export interface PresentationWindowPayload {
  route: string;
  windowTitle: string;
  plotType?: string;
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
    title: presentation.windowTitle,
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
