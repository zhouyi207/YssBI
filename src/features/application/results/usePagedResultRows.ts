import { useCallback, useEffect, useState, useRef, useSyncExternalStore } from "react";

import type { ErrorReference } from "@/features/application/errorReference";
import type { DeepReadonly } from "@/shared/types/deepReadonly";
import type { ResultPage } from "@/shared/types/domain/result";
import type {
  ResultPageRequest,
  ResultQueryCoordinator,
  ResultQueryOutcome,
  ResultQueryReadCapability,
} from "./resultQueryCoordinator";
import { resultQueryCoordinator, resultQueryRead } from "./runtime";

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
  readonly totalCount: number | null;
  readonly actualCount: number;
  readonly hasMore: boolean;
  readonly columns: readonly { readonly name: string; readonly type: string }[];
  readonly rows: readonly (readonly unknown[])[];
  readonly values: readonly unknown[];
  readonly loading: boolean;
  readonly error: DeepReadonly<ErrorReference> | null;
  readonly pageIndex: number;
  readonly pageSize: number;
  readonly totalPages: number | null;
  readonly goToPage: (pageIndex: number) => void;
  readonly goToPreviousPage: () => void;
  readonly goToNextPage: () => void;
  readonly reload: () => Promise<ResultQueryOutcome>;
}

function rowsFromPage(page: DeepReadonly<ResultPage> | null): readonly (readonly unknown[])[] {
  if (!page || page.valueKind !== "sequence") return EMPTY_ROWS;
  return page.values.map((value) => (Array.isArray(value) ? value : [value]));
}

export function usePagedResultRows(
  resultId: string | null,
  totalCount: number | null,
  pageSize = DEFAULT_PAGE_SIZE,
  dependencies: PagedResultRowsHookDependencies = {
    coordinator: resultQueryCoordinator,
    read: resultQueryRead,
  },
): PagedResultRowsState {
  const safePageSize = Math.max(1, Math.floor(pageSize));
  const [pageIndex, setPageIndex] = useState(0);
  const [loading, setLoading] = useState(false);
  const requestGeneration = useRef(0);
  const requestedOffset = pageIndex * safePageSize;
  const request: ResultPageRequest = {
    resultId: resultId ?? "",
    offset: requestedOffset,
    limit: safePageSize,
  };
  const page = useSyncExternalStore(
    dependencies.read.subscribe,
    () => (resultId === null ? null : dependencies.read.getPage(request)),
    () => (resultId === null ? null : dependencies.read.getPage(request)),
  );
  const effectiveTotalCount =
    page?.totalCount ?? (totalCount === null ? null : Math.max(0, totalCount));
  const totalPages =
    effectiveTotalCount === null
      ? null
      : Math.max(1, Math.ceil(effectiveTotalCount / safePageSize));
  const boundedPageIndex = totalPages === null ? pageIndex : Math.min(pageIndex, totalPages - 1);
  const hasMore = page?.hasMore ?? (totalPages !== null && boundedPageIndex < totalPages - 1);
  const error =
    resultId === null ? null : dependencies.read.getFailure({ kind: "page", ...request });

  useEffect(() => {
    setPageIndex(0);
  }, [resultId, totalCount, safePageSize]);

  useEffect(() => {
    const releasePayload =
      resultId === null ? undefined : dependencies.coordinator.retainPayload(resultId);
    return () => {
      requestGeneration.current += 1;
      releasePayload?.();
    };
  }, [dependencies.coordinator, resultId]);

  const loadPage = useCallback(
    async (nextPageIndex: number): Promise<ResultQueryOutcome> => {
      if (resultId === null) return { status: "notReady" };
      const nextOffset = Math.max(0, nextPageIndex) * safePageSize;
      const generation = ++requestGeneration.current;
      setLoading(true);
      try {
        const outcome = await dependencies.coordinator.loadPage({
          resultId,
          offset: nextOffset,
          limit: safePageSize,
        });
        return outcome;
      } finally {
        if (requestGeneration.current === generation) setLoading(false);
      }
    },
    [dependencies.coordinator, resultId, safePageSize],
  );

  useEffect(() => {
    if (resultId === null || totalCount === 0) {
      setLoading(false);
      return;
    }
    void loadPage(boundedPageIndex);
  }, [boundedPageIndex, totalCount, loadPage, resultId]);

  const goToPage = useCallback(
    (nextPageIndex: number): void => {
      const upper = totalPages === null ? boundedPageIndex + Number(hasMore) : totalPages - 1;
      const clamped = Math.max(0, Math.min(nextPageIndex, upper));
      setPageIndex(clamped);
    },
    [totalPages, boundedPageIndex, hasMore],
  );

  const reload = useCallback(
    async (): Promise<ResultQueryOutcome> => loadPage(boundedPageIndex),
    [boundedPageIndex, loadPage],
  );

  return {
    offset: page?.offset ?? requestedOffset,
    limit: page?.requestedLimit ?? safePageSize,
    totalCount: effectiveTotalCount,
    actualCount: page?.actualCount ?? 0,
    hasMore,
    columns: page?.metadata && "columns" in page.metadata ? page.metadata.columns : [],
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
