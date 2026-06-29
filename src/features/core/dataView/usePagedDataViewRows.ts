import { useCallback, useEffect, useRef, useState } from 'react';
import { DATA_VIEW_CHUNK_SIZE } from '@/app/appConfig/default';
import { DataViewService } from './dataViewService';
import type { DataViewPageState } from './types';

const EMPTY_ROWS: unknown[][] = [];
const EMPTY_VALUES: unknown[] = [];

export function usePagedDataViewRows(
  sourceId: string | null,
  totalCount: number,
  pageSize = DATA_VIEW_CHUNK_SIZE,
) {
  const [state, setState] = useState<DataViewPageState>({
    offset: 0,
    limit: pageSize,
    totalCount,
    rows: EMPTY_ROWS,
    values: EMPTY_VALUES,
    loading: false,
    error: null,
  });

  const requestRef = useRef(0);
  const sourceIdRef = useRef(sourceId);
  sourceIdRef.current = sourceId;

  const loadPage = useCallback(
    async (offset: number) => {
      if (!sourceId) return;
      const requestId = ++requestRef.current;
      const maxOffset = totalCount > 0 ? Math.max(0, totalCount - 1) : 0;
      const safeOffset = Math.max(0, Math.min(offset, maxOffset));
      setState((prev) => ({ ...prev, loading: true, error: null }));

      try {
        const page = await DataViewService.getPage(sourceId, safeOffset, pageSize);
        if (requestId !== requestRef.current || sourceIdRef.current !== sourceId) return;
        setState({
          offset: page.offset,
          limit: page.limit,
          totalCount: page.totalCount,
          rows: page.rows ?? EMPTY_ROWS,
          values: page.values ?? EMPTY_VALUES,
          loading: false,
          error: null,
        });
      } catch (e) {
        if (requestId !== requestRef.current) return;
        setState((prev) => ({
          ...prev,
          loading: false,
          error: e instanceof Error ? e.message : String(e),
        }));
      }
    },
    [sourceId, pageSize, totalCount],
  );

  useEffect(() => {
    if (!sourceId || totalCount <= 0) {
      setState((prev) => ({
        ...prev,
        totalCount,
        rows: EMPTY_ROWS,
        values: EMPTY_VALUES,
        loading: false,
        error: null,
      }));
      return;
    }
    void loadPage(0);
  }, [sourceId, totalCount, loadPage]);

  const totalPages = Math.max(1, Math.ceil((state.totalCount || totalCount) / pageSize));
  const pageIndex = Math.floor(state.offset / pageSize);

  const goToPage = useCallback(
    (nextPageIndex: number) => {
      const clamped = Math.max(0, Math.min(nextPageIndex, totalPages - 1));
      void loadPage(clamped * pageSize);
    },
    [loadPage, pageSize, totalPages],
  );

  return {
    ...state,
    pageIndex,
    pageSize,
    totalPages,
    goToPage,
    goToPreviousPage: () => goToPage(pageIndex - 1),
    goToNextPage: () => goToPage(pageIndex + 1),
    reload: () => void loadPage(state.offset),
  };
}
