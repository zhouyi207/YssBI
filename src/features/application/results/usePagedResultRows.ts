import { useCallback, useEffect, useState, useSyncExternalStore } from 'react';

import type { ErrorReference } from '@/services/ipc';
import type { DeepReadonly } from '@/features/core/projection/deepReadonly';
import type { ResultPage } from '@/shared/types/dto/result';
import type {
  ResultPageRequest,
  ResultQueryCoordinator,
  ResultQueryOutcome,
  ResultQueryReadCapability,
} from './resultQueryCoordinator';

const DEFAULT_PAGE_SIZE = 200;
const EMPTY_ROWS: readonly unknown[][] = [];
const EMPTY_VALUES: readonly unknown[] = [];

export interface PagedResultRowsHookDependencies {
  readonly coordinator: ResultQueryCoordinator;
  readonly read: ResultQueryReadCapability;
}

export interface PagedResultRowsState {
  readonly offset: number;
  readonly limit: number;
  readonly totalCount: number;
  readonly rows: readonly (readonly unknown[])[];
  readonly values: readonly unknown[];
  readonly loading: boolean;
  readonly error: DeepReadonly<ErrorReference> | null;
  readonly pageIndex: number;
  readonly pageSize: number;
  readonly totalPages: number;
  readonly goToPage: (pageIndex: number) => void;
  readonly goToPreviousPage: () => void;
  readonly goToNextPage: () => void;
  readonly reload: () => Promise<ResultQueryOutcome>;
}

function rowsFromPage(page: DeepReadonly<ResultPage> | null): readonly (readonly unknown[])[] {
  if (!page || page.valueKind !== 'sequence') return EMPTY_ROWS;
  return page.values.map((value) => Array.isArray(value) ? value : [value]);
}

export function usePagedResultRows(
  resultId: string | null,
  totalCount: number,
  pageSize = DEFAULT_PAGE_SIZE,
  dependencies: PagedResultRowsHookDependencies,
): PagedResultRowsState {
  const safePageSize = Math.max(1, Math.floor(pageSize));
  const [pageIndex, setPageIndex] = useState(0);
  const [loading, setLoading] = useState(false);
  const [loadedPage, setLoadedPage] = useState<DeepReadonly<ResultPage> | null>(null);
  const requestedOffset = pageIndex * safePageSize;
  const request: ResultPageRequest = {
    resultId: resultId ?? '',
    offset: requestedOffset,
    limit: safePageSize,
  };
  const projectedPage = useSyncExternalStore(
    dependencies.read.subscribe,
    () => resultId === null ? null : dependencies.read.getPage(request),
    () => resultId === null ? null : dependencies.read.getPage(request),
  );
  const page = projectedPage ?? loadedPage;
  const effectiveTotalCount = page?.totalCount ?? Math.max(0, totalCount);
  const totalPages = Math.max(1, Math.ceil(effectiveTotalCount / safePageSize));
  const boundedPageIndex = Math.min(pageIndex, totalPages - 1);
  const error = resultId === null
    ? null
    : dependencies.read.getFailure({ kind: 'page', ...request });

  useEffect(() => {
    setPageIndex(0);
    setLoadedPage(null);
  }, [resultId, totalCount, safePageSize]);

  const loadPage = useCallback(async (nextPageIndex: number): Promise<ResultQueryOutcome> => {
    if (resultId === null) return { status: 'notReady' };
    const nextOffset = Math.max(0, nextPageIndex) * safePageSize;
    setLoading(true);
    try {
      const outcome = await dependencies.coordinator.loadPage({
        resultId,
        offset: nextOffset,
        limit: safePageSize,
      });
      if (outcome.status === 'published') {
        setLoadedPage(dependencies.read.getPage({
          resultId,
          offset: nextOffset,
          limit: safePageSize,
        }));
      }
      return outcome;
    } finally {
      setLoading(false);
    }
  }, [dependencies.coordinator, dependencies.read, resultId, safePageSize]);

  useEffect(() => {
    if (resultId === null || effectiveTotalCount <= 0) {
      setLoading(false);
      return;
    }
    void loadPage(boundedPageIndex);
  }, [boundedPageIndex, effectiveTotalCount, loadPage, resultId]);

  const goToPage = useCallback((nextPageIndex: number): void => {
    const clamped = Math.max(0, Math.min(nextPageIndex, totalPages - 1));
    setPageIndex(clamped);
  }, [totalPages]);

  const reload = useCallback(async (): Promise<ResultQueryOutcome> => (
    loadPage(boundedPageIndex)
  ), [boundedPageIndex, loadPage]);

  return {
    offset: page?.offset ?? requestedOffset,
    limit: page?.requestedLimit ?? safePageSize,
    totalCount: effectiveTotalCount,
    rows: rowsFromPage(page),
    values: page?.values ?? EMPTY_VALUES,
    loading,
    error,
    pageIndex: boundedPageIndex,
    pageSize: safePageSize,
    totalPages,
    goToPage,
    goToPreviousPage: () => goToPage(boundedPageIndex - 1),
    goToNextPage: () => goToPage(boundedPageIndex + 1),
    reload,
  };
}
