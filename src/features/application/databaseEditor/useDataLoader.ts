import { useState, useRef, useCallback, useEffect } from "react";
import { useDatabaseRead } from "@/features/core/database/read";
import { initializeProjectForCurrentWindow as initProjectSync } from "@/features/application/project";
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
  elapsed: number;
}

export function useDataLoader(selectedDfId: string | null) {
  const selectedRowCount = useDatabaseRead((s) =>
    selectedDfId ? (s.databases[selectedDfId]?.rowCount ?? 0) : 0,
  );
  const [page, setPage] = useState<LoadedPage | null>(null);
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

  const loadPage = useCallback(
    async (id: string | null, nextPageIndex: number, mode: "page" | "data" | "project") => {
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
        if (mode === "project") await initProjectSync();
        if (!isActive() || !id) return;
        read = captureDatabaseRead(identity, id, isActive);
        if (mode !== "page") {
          try {
            await readDatabaseMetadata(read);
          } catch (error) {
            if (!read.isCurrent()) return;
            logger.data.warn(
              "getDatabaseMeta failed before row load: " + formatApplicationIpcError(error),
              "DatabaseEditorWindow",
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
          elapsed: Math.round(performance.now() - startedAt),
        });
      } catch (error) {
        if (read ? read.isCurrent() : isActive()) {
          logger.data.error(
            "Failed to load database page: " + formatApplicationIpcError(error),
            "DatabaseEditorWindow",
          );
        }
      } finally {
        if (isActive()) setLoading(false);
      }
    },
    [],
  );

  const loadInitialRows = useCallback((id: string) => loadPage(id, 0, "data"), [loadPage]);
  const reloadAllData = useCallback(async () => {
    if (selectedDfId) await loadPage(selectedDfId, pageIndex, "data");
  }, [selectedDfId, pageIndex, loadPage]);
  const refreshData = useCallback(async () => {
    await loadPage(selectedDfId, pageIndex, "project");
  }, [selectedDfId, pageIndex, loadPage]);
  const goToPage = useCallback(
    async (nextPageIndex: number) => {
      if (selectedDfId) await loadPage(selectedDfId, nextPageIndex, "page");
    },
    [selectedDfId, loadPage],
  );
  const goToPreviousPage = useCallback(() => goToPage(pageIndex - 1), [goToPage, pageIndex]);
  const goToNextPage = useCallback(() => goToPage(pageIndex + 1), [goToPage, pageIndex]);
  const clearData = useCallback(() => {
    requestEpochRef.current += 1;
    setPage(null);
    setPageIndex(0);
    setLoading(false);
  }, []);
  const visible = page?.read.isCurrent() ? page : null;

  return {
    loadedRows: visible?.rows ?? EMPTY_ROWS,
    loadedRowIds: visible?.rowIds ?? EMPTY_ROW_IDS,
    loading,
    pageIndex,
    pageSize: DATABASE_EDITOR_CHUNK_SIZE,
    pageStartIndex: pageIndex * DATABASE_EDITOR_CHUNK_SIZE,
    lastFetchMs: visible?.elapsed ?? null,
    totalPages: Math.max(1, Math.ceil(selectedRowCount / DATABASE_EDITOR_CHUNK_SIZE)),
    clearData,
    loadInitialRows,
    reloadAllData,
    goToPage,
    goToPreviousPage,
    goToNextPage,
    refreshData,
  };
}
