import type { ChartDocument, ChartPreviewPayload } from "@/shared/types/domain";

const MAX_CACHE_ENTRIES = 32;

type PreviewLoader = () => Promise<ChartPreviewPayload>;

const previewCache = new Map<string, ChartPreviewPayload>();
const inFlight = new Map<string, Promise<ChartPreviewPayload>>();
const databaseKeys = new Map<string, Set<string>>();
const chartKeys = new Map<string, Set<string>>();
const keyOwners = new Map<string, { databaseId: string; chartOwner: string }>();
let cacheGeneration = 0;

function stableEncodingsKey(encodings: ChartDocument["encodings"]): string {
  return JSON.stringify({
    x: encodings.x ?? null,
    y: encodings.y ?? null,
  });
}

export function chartPreviewCacheKey(
  projectInstanceId: string,
  chartPath: string,
  document: ChartDocument,
): string {
  return JSON.stringify({
    projectInstanceId,
    chartPath,
    databaseId: document.databaseId,
    chartType: document.chartType,
    encodings: stableEncodingsKey(document.encodings),
  });
}

export function getCachedChartPreview(
  projectInstanceId: string,
  chartPath: string,
  document: ChartDocument,
): ChartPreviewPayload | undefined {
  const key = chartPreviewCacheKey(projectInstanceId, chartPath, document);
  const cached = previewCache.get(key);
  if (!cached) return undefined;
  previewCache.delete(key);
  previewCache.set(key, cached);
  return cached;
}

function chartOwnerKey(projectInstanceId: string, chartPath: string): string {
  return JSON.stringify({ projectInstanceId, chartPath });
}

function rememberChartKey(projectInstanceId: string, chartPath: string, key: string): void {
  const owner = chartOwnerKey(projectInstanceId, chartPath);
  const keys = chartKeys.get(owner) ?? new Set<string>();
  keys.add(key);
  chartKeys.set(owner, keys);
}

function rememberDatabaseKey(databaseId: string, key: string) {
  if (!databaseKeys.has(databaseId)) {
    databaseKeys.set(databaseId, new Set());
  }
  databaseKeys.get(databaseId)!.add(key);
}

function rememberKeyOwners(
  projectInstanceId: string,
  chartPath: string,
  databaseId: string,
  key: string,
): void {
  const chartOwner = chartOwnerKey(projectInstanceId, chartPath);
  rememberDatabaseKey(databaseId, key);
  rememberChartKey(projectInstanceId, chartPath, key);
  keyOwners.set(key, { databaseId, chartOwner });
}

function forgetKeyOwners(key: string): void {
  const owners = keyOwners.get(key);
  if (!owners) return;
  const databaseOwnerKeys = databaseKeys.get(owners.databaseId);
  databaseOwnerKeys?.delete(key);
  if (databaseOwnerKeys?.size === 0) databaseKeys.delete(owners.databaseId);
  const chartOwnerKeys = chartKeys.get(owners.chartOwner);
  chartOwnerKeys?.delete(key);
  if (chartOwnerKeys?.size === 0) chartKeys.delete(owners.chartOwner);
  keyOwners.delete(key);
}

function removeKey(key: string): void {
  previewCache.delete(key);
  inFlight.delete(key);
  forgetKeyOwners(key);
}

function writeCache(key: string, preview: ChartPreviewPayload) {
  if (preview.kind === "error") return;

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

export async function getChartPreview(
  projectInstanceId: string,
  chartPath: string,
  document: ChartDocument,
  loader: PreviewLoader,
): Promise<ChartPreviewPayload> {
  const cached = getCachedChartPreview(projectInstanceId, chartPath, document);
  if (cached) {
    return cached;
  }

  const key = chartPreviewCacheKey(projectInstanceId, chartPath, document);
  const pending = inFlight.get(key);
  if (pending) return pending;

  const generation = cacheGeneration;
  let request!: Promise<ChartPreviewPayload>;
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
  rememberKeyOwners(projectInstanceId, chartPath, document.databaseId, key);
  return request;
}

function invalidateKeys(keys: ReadonlySet<string> | undefined): void {
  [...(keys ?? [])].forEach(removeKey);
}

export function invalidateChartPreviewCacheForMove(
  projectInstanceId: string,
  from: string,
  to: string,
): void {
  for (const chartPath of new Set([from, to])) {
    const owner = chartOwnerKey(projectInstanceId, chartPath);
    invalidateKeys(chartKeys.get(owner));
    chartKeys.delete(owner);
  }
}

export function invalidateChartPreviewCacheForDatabase(databaseId: string) {
  const keys = databaseKeys.get(databaseId);
  if (!keys) return;
  invalidateKeys(keys);
  databaseKeys.delete(databaseId);
}

export function clearChartPreviewCache() {
  cacheGeneration += 1;
  previewCache.clear();
  inFlight.clear();
  databaseKeys.clear();
  chartKeys.clear();
  keyOwners.clear();
}

export function getChartPreviewCacheSnapshotForTests() {
  return {
    previewKeys: new Set(previewCache.keys()),
    inFlightKeys: new Set(inFlight.keys()),
    databaseKeys: new Map([...databaseKeys].map(([owner, keys]) => [owner, new Set(keys)])),
    chartKeys: new Map([...chartKeys].map(([owner, keys]) => [owner, new Set(keys)])),
    keyOwnerKeys: new Set(keyOwners.keys()),
  };
}
