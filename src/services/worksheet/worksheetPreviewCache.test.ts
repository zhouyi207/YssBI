import { describe, expect, it } from 'vitest';
import type { WorksheetDocument, WorksheetPreviewPayload } from '@/shared/types/domain';
import {
  clearWorksheetPreviewCache,
  getWorksheetPreview,
  getCachedWorksheetPreview,
  invalidateWorksheetPreviewCacheForDatabase,
  worksheetPreviewCacheKey,
} from './worksheetPreviewCache';

const PROJECT_A = '00000000-0000-0000-0000-000000000601';
const PROJECT_B = '00000000-0000-0000-0000-000000000602';

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((settle, fail) => {
    resolve = settle;
    reject = fail;
  });
  return { promise, resolve, reject };
}

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
    const left = worksheetPreviewCacheKey(PROJECT_A, {
      ...DOCUMENT,
      encodings: { x: 'x', y: 'y' },
    });
    const right = worksheetPreviewCacheKey(PROJECT_A, {
      ...DOCUMENT,
      encodings: { y: 'y', x: 'x' },
    });

    expect(left).toBe(right);
  });

  it('separates the same worksheet and database IDs by project identity', () => {
    expect(worksheetPreviewCacheKey(PROJECT_A, DOCUMENT))
      .not.toBe(worksheetPreviewCacheKey(PROJECT_B, DOCUMENT));
  });
});

describe('getWorksheetPreview', () => {
  it('reuses completed previews for the same worksheet spec', async () => {
    clearWorksheetPreviewCache();
    let calls = 0;
    const preview: WorksheetPreviewPayload = { kind: 'empty' };

    const first = await getWorksheetPreview(PROJECT_A, DOCUMENT, async () => {
      calls += 1;
      return preview;
    });
    const second = await getWorksheetPreview(PROJECT_A, DOCUMENT, async () => {
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

    expect(getCachedWorksheetPreview(PROJECT_A, DOCUMENT)).toBeUndefined();
    await getWorksheetPreview(PROJECT_A, DOCUMENT, async () => preview);

    expect(getCachedWorksheetPreview(PROJECT_A, DOCUMENT)).toBe(preview);
  });

  it('deduplicates concurrent preview requests for the same worksheet spec', async () => {
    clearWorksheetPreviewCache();
    let calls = 0;
    const preview: WorksheetPreviewPayload = { kind: 'empty' };

    const [first, second] = await Promise.all([
      getWorksheetPreview(PROJECT_A, DOCUMENT, async () => {
        calls += 1;
        return preview;
      }),
      getWorksheetPreview(PROJECT_A, DOCUMENT, async () => {
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

    await getWorksheetPreview(PROJECT_A, DOCUMENT, async () => {
      calls += 1;
      return { kind: 'empty' };
    });

    invalidateWorksheetPreviewCacheForDatabase(DOCUMENT.databaseId);

    await getWorksheetPreview(PROJECT_A, DOCUMENT, async () => {
      calls += 1;
      return { kind: 'empty' };
    });

    expect(calls).toBe(2);
  });

  it.each(['old-first', 'new-first'] as const)(
    'keeps the post-invalidation request authoritative when requests settle %s',
    async (settlementOrder) => {
      clearWorksheetPreviewCache();
      const oldRequest = deferred<WorksheetPreviewPayload>();
      const newRequest = deferred<WorksheetPreviewPayload>();
      const oldPayload: WorksheetPreviewPayload = { kind: 'empty' };
      const newPayload: WorksheetPreviewPayload = { kind: 'line', pair: {
        data: [{ x: 8, y: 9 }], xLabel: 'x', yLabel: 'y', xFormat: 'number', yFormat: 'number',
      } };
      const oldCompletion = getWorksheetPreview(PROJECT_A, DOCUMENT, () => oldRequest.promise);

      invalidateWorksheetPreviewCacheForDatabase(DOCUMENT.databaseId);
      let newLoaderCalls = 0;
      const newCompletion = getWorksheetPreview(PROJECT_A, DOCUMENT, () => {
        newLoaderCalls += 1;
        return newRequest.promise;
      });
      expect(newLoaderCalls).toBe(1);

      if (settlementOrder === 'old-first') {
        oldRequest.resolve(oldPayload);
        await expect(oldCompletion).resolves.toBe(oldPayload);
        expect(getCachedWorksheetPreview(PROJECT_A, DOCUMENT)).toBeUndefined();

        let thirdLoaderCalls = 0;
        const sharedCompletion = getWorksheetPreview(PROJECT_A, DOCUMENT, async () => {
          thirdLoaderCalls += 1;
          return oldPayload;
        });
        expect(thirdLoaderCalls).toBe(0);

        newRequest.resolve(newPayload);
        await expect(sharedCompletion).resolves.toBe(newPayload);
      } else {
        newRequest.resolve(newPayload);
        await expect(newCompletion).resolves.toBe(newPayload);
        oldRequest.resolve(oldPayload);
        await expect(oldCompletion).resolves.toBe(oldPayload);
      }

      await expect(newCompletion).resolves.toBe(newPayload);
      expect(getCachedWorksheetPreview(PROJECT_A, DOCUMENT)).toBe(newPayload);
    },
  );

  it('prevents an invalidated in-flight request from writing without a replacement', async () => {
    clearWorksheetPreviewCache();
    const oldRequest = deferred<WorksheetPreviewPayload>();
    const oldPayload: WorksheetPreviewPayload = { kind: 'empty' };
    const oldCompletion = getWorksheetPreview(PROJECT_A, DOCUMENT, () => oldRequest.promise);

    invalidateWorksheetPreviewCacheForDatabase(DOCUMENT.databaseId);
    oldRequest.resolve(oldPayload);

    await expect(oldCompletion).resolves.toBe(oldPayload);
    expect(getCachedWorksheetPreview(PROJECT_A, DOCUMENT)).toBeUndefined();
  });

  it('does not reuse a completed preview across replacement projects with the same IDs', async () => {
    clearWorksheetPreviewCache();
    const original: WorksheetPreviewPayload = { kind: 'scatter', pair: {
      data: [{ x: 1, y: 1 }], xLabel: 'x', yLabel: 'y', xFormat: 'number', yFormat: 'number',
    } };
    const replacement: WorksheetPreviewPayload = { kind: 'scatter', pair: {
      data: [{ x: 2, y: 2 }], xLabel: 'x', yLabel: 'y', xFormat: 'number', yFormat: 'number',
    } };

    await getWorksheetPreview(PROJECT_A, DOCUMENT, async () => original);
    const result = await getWorksheetPreview(PROJECT_B, DOCUMENT, async () => replacement);

    expect(result).toBe(replacement);
    expect(getCachedWorksheetPreview(PROJECT_A, DOCUMENT)).toBe(original);
    expect(getCachedWorksheetPreview(PROJECT_B, DOCUMENT)).toBe(replacement);
  });

  it('does not reuse an in-flight preview across replacement projects with the same IDs', async () => {
    clearWorksheetPreviewCache();
    const oldRequest = deferred<WorksheetPreviewPayload>();
    const replacement: WorksheetPreviewPayload = { kind: 'line', pair: {
      data: [{ x: 2, y: 3 }], xLabel: 'x', yLabel: 'y', xFormat: 'number', yFormat: 'number',
    } };
    const oldCompletion = getWorksheetPreview(PROJECT_A, DOCUMENT, () => oldRequest.promise);

    const replacementResult = await getWorksheetPreview(
      PROJECT_B,
      DOCUMENT,
      async () => replacement,
    );
    oldRequest.resolve({ kind: 'empty' });
    await oldCompletion;

    expect(replacementResult).toBe(replacement);
    expect(getCachedWorksheetPreview(PROJECT_B, DOCUMENT)).toBe(replacement);
  });

  it('keeps a newer same-key request authoritative when a cleared request resolves', async () => {
    clearWorksheetPreviewCache();
    const oldRequest = deferred<WorksheetPreviewPayload>();
    const newRequest = deferred<WorksheetPreviewPayload>();
    const oldPayload: WorksheetPreviewPayload = { kind: 'empty' };
    const newPayload: WorksheetPreviewPayload = { kind: 'line', pair: {
      data: [{ x: 4, y: 5 }], xLabel: 'x', yLabel: 'y', xFormat: 'number', yFormat: 'number',
    } };
    const oldCompletion = getWorksheetPreview(PROJECT_A, DOCUMENT, () => oldRequest.promise);

    clearWorksheetPreviewCache();
    const newCompletion = getWorksheetPreview(PROJECT_A, DOCUMENT, () => newRequest.promise);
    oldRequest.resolve(oldPayload);
    await oldCompletion;

    expect(getCachedWorksheetPreview(PROJECT_A, DOCUMENT)).toBeUndefined();
    let replacementLoaderCalls = 0;
    const sharedCompletion = getWorksheetPreview(PROJECT_A, DOCUMENT, async () => {
      replacementLoaderCalls += 1;
      return oldPayload;
    });
    expect(replacementLoaderCalls).toBe(0);

    newRequest.resolve(newPayload);
    await expect(newCompletion).resolves.toBe(newPayload);
    await expect(sharedCompletion).resolves.toBe(newPayload);
    expect(getCachedWorksheetPreview(PROJECT_A, DOCUMENT)).toBe(newPayload);
  });

  it('keeps a newer same-key request authoritative when a cleared request rejects', async () => {
    clearWorksheetPreviewCache();
    const oldRequest = deferred<WorksheetPreviewPayload>();
    const newRequest = deferred<WorksheetPreviewPayload>();
    const newPayload: WorksheetPreviewPayload = { kind: 'scatter', pair: {
      data: [{ x: 6, y: 7 }], xLabel: 'x', yLabel: 'y', xFormat: 'number', yFormat: 'number',
    } };
    const oldCompletion = getWorksheetPreview(PROJECT_A, DOCUMENT, () => oldRequest.promise);

    clearWorksheetPreviewCache();
    const newCompletion = getWorksheetPreview(PROJECT_A, DOCUMENT, () => newRequest.promise);
    oldRequest.reject(new Error('stale request failed'));
    await expect(oldCompletion).rejects.toThrow('stale request failed');

    expect(getCachedWorksheetPreview(PROJECT_A, DOCUMENT)).toBeUndefined();
    let replacementLoaderCalls = 0;
    const sharedCompletion = getWorksheetPreview(PROJECT_A, DOCUMENT, async () => {
      replacementLoaderCalls += 1;
      return { kind: 'empty' };
    });
    expect(replacementLoaderCalls).toBe(0);

    newRequest.resolve(newPayload);
    await expect(newCompletion).resolves.toBe(newPayload);
    await expect(sharedCompletion).resolves.toBe(newPayload);
    expect(getCachedWorksheetPreview(PROJECT_A, DOCUMENT)).toBe(newPayload);
  });
});
