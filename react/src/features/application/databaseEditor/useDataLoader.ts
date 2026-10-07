import { useState, useRef, useCallback, useEffect } from "react";
import { useDatabaseRead } from "@/features/core/database/read";
import { useResourceStore } from "@/features/core/resource/resourceStore";
import { DATABASE_EDITOR_CHUNK_SIZE } from "@/shared/config-default";
import type { DatabaseRowsResult } from "@/services/database/databaseService";
import { logger } from "@/utils/frontendLogger";
import { formatApplicationIpcError } from "@/features/application/errorReference";
import {
  captureProjectIdentity,
  isCurrentProjectIdentity,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import {
  captureDatabaseRead,
  readDatabaseMetadata,
  readDatabasePage,
  type DatabaseRead,
} from "@/features/application/dataManagement/databaseRead";

const EMPTY_ROWS: DatabaseRowsResult["rows"] = [];
const EMPTY_ROW_IDS: DatabaseRowsResult["rowIds"] = [];

interface LoadedPage extends DatabaseRowsResult {
  read: DatabaseRead;
  dataRevision: string | undefined;
  elapsed: number;
}

function isPageCurrent(page: LoadedPage): boolean {
  return page.dataRevision === undefined
    ? page.read.isCurrent()
    : page.read.isDataCurrent(page.dataRevision);
}

export function useDataLoader(selectedDfId: string) {
  const selectedRowCount = useDatabaseRead((s) => s.databases[selectedDfId]?.rowCount ?? 0);
  const [page, setPage] = useState<LoadedPage | null>(null);
  const pageRef = useRef(page);
  pageRef.current = page;
  const [loading, setLoading] = useState(true);
  const [pageIndex, setPageIndex] = useState(0);
  const requestEpochRef = useRef(0);
  const mountedRef = useRef(true);
  const selectedDfIdRef = useRef(selectedDfId);
  selectedDfIdRef.current = selectedDfId;

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      requestEpochRef.current += 1;
    };
  }, []);

  const loadPage = useCallback(async (id: string, nextPageIndex: number, mode: "page" | "data") => {
    const identity = captureProjectIdentity();
    const epoch = ++requestEpochRef.current;
    const isActive = () =>
      mountedRef.current &&
      epoch === requestEpochRef.current &&
      id === selectedDfIdRef.current &&
      isCurrentProjectIdentity(identity);
    if (!isActive()) return;
    setLoading(true);
    const startedAt = performance.now();
    let read: DatabaseRead | undefined;
    try {
      if (!isActive()) return;
      read = captureDatabaseRead(identity, id, isActive);
      if (mode !== "page") {
        try {
          await readDatabaseMetadata(read);
        } catch (error) {
          if (!read.isCurrent()) return;
          logger.data.warn(
            "getDatabaseMeta failed before row load: " + formatApplicationIpcError(error),
            "DatabaseEditor",
          );
        }
      }
      const result = await readDatabasePage(read, nextPageIndex, DATABASE_EDITOR_CHUNK_SIZE);
      if (!result || !read.isCurrent()) return;
      setPageIndex(result.pageIndex);
      setPage({
        rows: result.rows,
        rowIds: result.rowIds,
        read,
        dataRevision: useResourceStore.getState().databases[id]?.dataRevision,
        elapsed: Math.round(performance.now() - startedAt),
      });
    } catch (error) {
      if (read ? read.isCurrent() : isActive()) {
        logger.data.error(
          "Failed to load database page: " + formatApplicationIpcError(error),
          "DatabaseEditor",
        );
      }
    } finally {
      if (isActive()) setLoading(false);
    }
  }, []);

  const loadInitialRows = useCallback(
    (id: string) => {
      const loaded = pageRef.current;
      if (loaded?.read.id === id && isPageCurrent(loaded)) return Promise.resolve();
      const metadataReady = useResourceStore.getState().databases[id]?.dataRevision !== undefined;
      return loadPage(id, 0, metadataReady ? "page" : "data");
    },
    [loadPage],
  );
  const reloadAllData = useCallback(async () => {
    await loadPage(selectedDfId, pageIndex, "data");
  }, [selectedDfId, pageIndex, loadPage]);
  const goToPage = useCallback(
    async (nextPageIndex: number) => {
      await loadPage(selectedDfId, nextPageIndex, "page");
    },
    [selectedDfId, loadPage],
  );
  const goToPreviousPage = useCallback(() => goToPage(pageIndex - 1), [goToPage, pageIndex]);
  const goToNextPage = useCallback(() => goToPage(pageIndex + 1), [goToPage, pageIndex]);
  const visible = page && isPageCurrent(page) ? page : null;

  return {
    loadedRows: visible?.rows ?? EMPTY_ROWS,
    loadedRowIds: visible?.rowIds ?? EMPTY_ROW_IDS,
    loading,
    pageIndex,
    pageSize: DATABASE_EDITOR_CHUNK_SIZE,
    pageStartIndex: pageIndex * DATABASE_EDITOR_CHUNK_SIZE,
    lastFetchMs: visible?.elapsed ?? null,
    totalPages: Math.max(1, Math.ceil(selectedRowCount / DATABASE_EDITOR_CHUNK_SIZE)),
    loadInitialRows,
    reloadAllData,
    goToPage,
    goToPreviousPage,
    goToNextPage,
  };
}
