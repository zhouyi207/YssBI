import type { WorksheetDocument, WorksheetPreviewPayload } from '@/shared/types/domain';

const MAX_CACHE_ENTRIES = 32;

type PreviewLoader = () => Promise<WorksheetPreviewPayload>;

const previewCache = new Map<string, WorksheetPreviewPayload>();
const inFlight = new Map<string, Promise<WorksheetPreviewPayload>>();
const databaseKeys = new Map<string, Set<string>>();

function stableEncodingsKey(encodings: WorksheetDocument['encodings']): string {
  return JSON.stringify({
    x: encodings.x ?? null,
    y: encodings.y ?? null,
  });
}

export function worksheetPreviewCacheKey(document: WorksheetDocument): string {
  return JSON.stringify({
    worksheetId: document.id,
    databaseId: document.databaseId,
    chartType: document.chartType,
    encodings: stableEncodingsKey(document.encodings),
  });
}

export function getCachedWorksheetPreview(
  document: WorksheetDocument,
): WorksheetPreviewPayload | undefined {
  const key = worksheetPreviewCacheKey(document);
  const cached = previewCache.get(key);
  if (!cached) return undefined;
  previewCache.delete(key);
  previewCache.set(key, cached);
  return cached;
}

function rememberDatabaseKey(databaseId: string, key: string) {
  if (!databaseKeys.has(databaseId)) {
    databaseKeys.set(databaseId, new Set());
  }
  databaseKeys.get(databaseId)!.add(key);
}

function writeCache(key: string, databaseId: string, preview: WorksheetPreviewPayload) {
  if (preview.kind === 'error') return;

  if (previewCache.has(key)) {
    previewCache.delete(key);
  }
  previewCache.set(key, preview);
  rememberDatabaseKey(databaseId, key);

  while (previewCache.size > MAX_CACHE_ENTRIES) {
    const oldestKey = previewCache.keys().next().value as string | undefined;
    if (!oldestKey) break;
    previewCache.delete(oldestKey);
  }
}

export async function getWorksheetPreview(
  document: WorksheetDocument,
  loader: PreviewLoader,
): Promise<WorksheetPreviewPayload> {
  const cached = getCachedWorksheetPreview(document);
  if (cached) {
    return cached;
  }

  const key = worksheetPreviewCacheKey(document);
  const pending = inFlight.get(key);
  if (pending) return pending;

  const request = loader()
    .then((preview) => {
      writeCache(key, document.databaseId, preview);
      return preview;
    })
    .finally(() => {
      inFlight.delete(key);
    });
  inFlight.set(key, request);
  return request;
}

export function invalidateWorksheetPreviewCacheForDatabase(databaseId: string) {
  const keys = databaseKeys.get(databaseId);
  if (!keys) return;
  keys.forEach((key) => {
    previewCache.delete(key);
    inFlight.delete(key);
  });
  databaseKeys.delete(databaseId);
}

export function clearWorksheetPreviewCache() {
  previewCache.clear();
  inFlight.clear();
  databaseKeys.clear();
}
