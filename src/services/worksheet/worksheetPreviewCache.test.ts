import { describe, expect, it } from 'vitest';
import type { WorksheetDocument, WorksheetPreviewPayload } from '@/shared/types/domain';
import {
  clearWorksheetPreviewCache,
  getWorksheetPreview,
  getCachedWorksheetPreview,
  invalidateWorksheetPreviewCacheForDatabase,
  worksheetPreviewCacheKey,
} from './worksheetPreviewCache';

const DOCUMENT: WorksheetDocument = {
  schemaVersion: 3,
  revision: 0,
  id: 'worksheet-1',
  name: 'Chart',
  databaseId: 'database-1',
  chartType: 'scatter',
  encodings: { x: 'x', y: 'y' },
};

describe('worksheetPreviewCacheKey', () => {
  it('is stable for equivalent encoding objects', () => {
    const left = worksheetPreviewCacheKey({
      ...DOCUMENT,
      encodings: { x: 'x', y: 'y' },
    });
    const right = worksheetPreviewCacheKey({
      ...DOCUMENT,
      encodings: { y: 'y', x: 'x' },
    });

    expect(left).toBe(right);
  });
});

describe('getWorksheetPreview', () => {
  it('reuses completed previews for the same worksheet spec', async () => {
    clearWorksheetPreviewCache();
    let calls = 0;
    const preview: WorksheetPreviewPayload = { kind: 'empty' };

    const first = await getWorksheetPreview(DOCUMENT, async () => {
      calls += 1;
      return preview;
    });
    const second = await getWorksheetPreview(DOCUMENT, async () => {
      calls += 1;
      return { kind: 'error', message: 'should not run' };
    });

    expect(first).toBe(preview);
    expect(second).toBe(preview);
    expect(calls).toBe(1);
  });

  it('exposes a synchronous cached preview for immediate remounts', async () => {
    clearWorksheetPreviewCache();
    const preview: WorksheetPreviewPayload = { kind: 'empty' };

    expect(getCachedWorksheetPreview(DOCUMENT)).toBeUndefined();
    await getWorksheetPreview(DOCUMENT, async () => preview);

    expect(getCachedWorksheetPreview(DOCUMENT)).toBe(preview);
  });

  it('deduplicates concurrent preview requests for the same worksheet spec', async () => {
    clearWorksheetPreviewCache();
    let calls = 0;
    const preview: WorksheetPreviewPayload = { kind: 'empty' };

    const [first, second] = await Promise.all([
      getWorksheetPreview(DOCUMENT, async () => {
        calls += 1;
        return preview;
      }),
      getWorksheetPreview(DOCUMENT, async () => {
        calls += 1;
        return { kind: 'error', message: 'should not run' };
      }),
    ]);

    expect(first).toBe(preview);
    expect(second).toBe(preview);
    expect(calls).toBe(1);
  });

  it('invalidates cached previews by database id', async () => {
    clearWorksheetPreviewCache();
    let calls = 0;

    await getWorksheetPreview(DOCUMENT, async () => {
      calls += 1;
      return { kind: 'empty' };
    });

    invalidateWorksheetPreviewCacheForDatabase(DOCUMENT.databaseId);

    await getWorksheetPreview(DOCUMENT, async () => {
      calls += 1;
      return { kind: 'empty' };
    });

    expect(calls).toBe(2);
  });
});
