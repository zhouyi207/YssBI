import { useCallback, useEffect, useRef, useState } from 'react';
import { ResultService } from '@/services/result/resultService';
import type { ResultPageState } from './types';

const DEFAULT_PAGE_SIZE = 200;
const EMPTY_ROWS: unknown[][] = [];
const EMPTY_VALUES: unknown[] = [];

export function usePagedResultRows(
  resultId: string | null,
  totalCount: number,
  pageSize = DEFAULT_PAGE_SIZE,
) {
  const [state, setState] = useState<ResultPageState>({
    offset: 0,
    limit: pageSize,
    totalCount,
    rows: EMPTY_ROWS,
    values: EMPTY_VALUES,
    loading: false,
    error: null,
  });
  const requestRef = useRef(0);
  const resultIdRef = useRef(resultId);
  resultIdRef.current = resultId;

  const loadPage = useCallback(async (offset: number) => {
    if (!resultId) return;
    const requestId = ++requestRef.current;
    const maxOffset = totalCount > 0 ? Math.max(0, totalCount - 1) : 0;
    const safeOffset = Math.max(0, Math.min(offset, maxOffset));
    setState((prev) => ({ ...prev, loading: true, error: null }));
    try {
      const page = await ResultService.getPage(resultId, safeOffset, pageSize);
      if (!page || requestId !== requestRef.current || resultIdRef.current !== resultId) return;
      setState({
        offset: page.offset,
        limit: page.requestedLimit,
        totalCount: page.totalCount,
        rows: page.valueKind === 'sequence'
          ? page.values.map((value) => Array.isArray(value) ? value : [value])
          : EMPTY_ROWS,
        values: page.values,
        loading: false,
        error: null,
      });
    } catch (cause) {
      if (requestId !== requestRef.current) return;
      setState((prev) => ({
        ...prev,
        loading: false,
        error: cause instanceof Error ? cause.message : String(cause),
      }));
    }
  }, [resultId, pageSize, totalCount]);

  useEffect(() => {
    if (!resultId || totalCount <= 0) {
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
  }, [resultId, totalCount, loadPage]);

  const totalPages = Math.max(1, Math.ceil((state.totalCount || totalCount) / pageSize));
  const pageIndex = Math.floor(state.offset / pageSize);
  const goToPage = useCallback((nextPageIndex: number) => {
    const clamped = Math.max(0, Math.min(nextPageIndex, totalPages - 1));
    void loadPage(clamped * pageSize);
  }, [loadPage, pageSize, totalPages]);

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
