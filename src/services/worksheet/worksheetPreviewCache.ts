import type { WorksheetDocument, WorksheetPreviewPayload } from '@/shared/types/domain';

const MAX_CACHE_ENTRIES = 32;

type PreviewLoader = () => Promise<WorksheetPreviewPayload>;

const previewCache = new Map<string, WorksheetPreviewPayload>();
const inFlight = new Map<string, Promise<WorksheetPreviewPayload>>();
const databaseKeys = new Map<string, Set<string>>();
const worksheetKeys = new Map<string, Set<string>>();
const keyOwners = new Map<string, { databaseId: string; worksheetOwner: string }>();
let cacheGeneration = 0;

function stableEncodingsKey(encodings: WorksheetDocument['encodings']): string {
  return JSON.stringify({
    x: encodings.x ?? null,
    y: encodings.y ?? null,
  });
}

export function worksheetPreviewCacheKey(
  projectInstanceId: string,
  worksheetPath: string,
  document: WorksheetDocument,
): string {
  return JSON.stringify({
    projectInstanceId,
    worksheetPath,
    databaseId: document.databaseId,
    chartType: document.chartType,
    encodings: stableEncodingsKey(document.encodings),
  });
}

export function getCachedWorksheetPreview(
  projectInstanceId: string,
  worksheetPath: string,
  document: WorksheetDocument,
): WorksheetPreviewPayload | undefined {
  const key = worksheetPreviewCacheKey(projectInstanceId, worksheetPath, document);
  const cached = previewCache.get(key);
  if (!cached) return undefined;
  previewCache.delete(key);
  previewCache.set(key, cached);
  return cached;
}

function worksheetOwnerKey(projectInstanceId: string, worksheetPath: string): string {
  return JSON.stringify({ projectInstanceId, worksheetPath });
}

function rememberWorksheetKey(projectInstanceId: string, worksheetPath: string, key: string): void {
  const owner = worksheetOwnerKey(projectInstanceId, worksheetPath);
  const keys = worksheetKeys.get(owner) ?? new Set<string>();
  keys.add(key);
  worksheetKeys.set(owner, keys);
}

function rememberDatabaseKey(databaseId: string, key: string) {
  if (!databaseKeys.has(databaseId)) {
    databaseKeys.set(databaseId, new Set());
  }
  databaseKeys.get(databaseId)!.add(key);
}

function rememberKeyOwners(
  projectInstanceId: string,
  worksheetPath: string,
  databaseId: string,
  key: string,
): void {
  const worksheetOwner = worksheetOwnerKey(projectInstanceId, worksheetPath);
  rememberDatabaseKey(databaseId, key);
  rememberWorksheetKey(projectInstanceId, worksheetPath, key);
  keyOwners.set(key, { databaseId, worksheetOwner });
}

function forgetKeyOwners(key: string): void {
  const owners = keyOwners.get(key);
  if (!owners) return;
  const databaseOwnerKeys = databaseKeys.get(owners.databaseId);
  databaseOwnerKeys?.delete(key);
  if (databaseOwnerKeys?.size === 0) databaseKeys.delete(owners.databaseId);
  const worksheetOwnerKeys = worksheetKeys.get(owners.worksheetOwner);
  worksheetOwnerKeys?.delete(key);
  if (worksheetOwnerKeys?.size === 0) worksheetKeys.delete(owners.worksheetOwner);
  keyOwners.delete(key);
}

function removeKey(key: string): void {
  previewCache.delete(key);
  inFlight.delete(key);
  forgetKeyOwners(key);
}

function writeCache(key: string, preview: WorksheetPreviewPayload) {
  if (preview.kind === 'error') return;

  if (previewCache.has(key)) {
    previewCache.delete(key);
  }
  previewCache.set(key, preview);

  while (previewCache.size > MAX_CACHE_ENTRIES) {
    const oldestKey = previewCache.keys().next().value as string | undefined;
    if (!oldestKey) break;
    removeKey(oldestKey);
  }
}

export async function getWorksheetPreview(
  projectInstanceId: string,
  worksheetPath: string,
  document: WorksheetDocument,
  loader: PreviewLoader,
): Promise<WorksheetPreviewPayload> {
  const cached = getCachedWorksheetPreview(projectInstanceId, worksheetPath, document);
  if (cached) {
    return cached;
  }

  const key = worksheetPreviewCacheKey(projectInstanceId, worksheetPath, document);
  const pending = inFlight.get(key);
  if (pending) return pending;

  const generation = cacheGeneration;
  let request!: Promise<WorksheetPreviewPayload>;
  request = loader()
    .then((preview) => {
      if (generation === cacheGeneration && inFlight.get(key) === request) {
        writeCache(key, preview);
      }
      return preview;
    })
    .finally(() => {
      if (generation === cacheGeneration && inFlight.get(key) === request) {
        inFlight.delete(key);
        if (!previewCache.has(key)) forgetKeyOwners(key);
      }
    });
  inFlight.set(key, request);
  rememberKeyOwners(projectInstanceId, worksheetPath, document.databaseId, key);
  return request;
}

function invalidateKeys(keys: ReadonlySet<string> | undefined): void {
  [...(keys ?? [])].forEach(removeKey);
}

export function invalidateWorksheetPreviewCacheForMove(
  projectInstanceId: string,
  from: string,
  to: string,
): void {
  for (const worksheetPath of new Set([from, to])) {
    const owner = worksheetOwnerKey(projectInstanceId, worksheetPath);
    invalidateKeys(worksheetKeys.get(owner));
    worksheetKeys.delete(owner);
  }
}

export function invalidateWorksheetPreviewCacheForDatabase(databaseId: string) {
  const keys = databaseKeys.get(databaseId);
  if (!keys) return;
  invalidateKeys(keys);
  databaseKeys.delete(databaseId);
}

export function clearWorksheetPreviewCache() {
  cacheGeneration += 1;
  previewCache.clear();
  inFlight.clear();
  databaseKeys.clear();
  worksheetKeys.clear();
  keyOwners.clear();
}

export function getWorksheetPreviewCacheSnapshotForTests() {
  return {
    previewKeys: new Set(previewCache.keys()),
    inFlightKeys: new Set(inFlight.keys()),
    databaseKeys: new Map([...databaseKeys].map(([owner, keys]) => [owner, new Set(keys)])),
    worksheetKeys: new Map([...worksheetKeys].map(([owner, keys]) => [owner, new Set(keys)])),
    keyOwnerKeys: new Set(keyOwners.keys()),
  };
}
