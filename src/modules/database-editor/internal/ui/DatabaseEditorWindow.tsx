import React, { useEffect, useState, useMemo, useRef } from "react";
import { useProjectSync } from "@/features/application/initialization";
import { initializeProjectForCurrentWindow } from "@/features/application/project";
import { useCurrentWindowActions, usePersistedWindow } from "@/features/application/window";
import { useDatabaseRead } from "@/features/core/database/read";
import { TitleBar, type DataframeOption } from "./Layout";
import { DatabaseEditorContent } from "./DatabaseEditorContent";
import { reportViewIssue } from "@/features/application/observability/reportViewIssue";

function getDatabaseIdFromUrl(): string | null {
  const searchValue = new URLSearchParams(window.location.search).get("database");
  if (searchValue) return searchValue;

  const hashQueryIndex = window.location.hash.indexOf("?");
  if (hashQueryIndex < 0) return null;
  return new URLSearchParams(window.location.hash.slice(hashQueryIndex + 1)).get("database");
}

export const DatabaseEditorWindow: React.FC = () => {
  const dataframes = useDatabaseRead((snapshot) => snapshot.databases);

  const [selectedDfId, setSelectedDfId] = useState<string | null>(null);
  const hasInitializedDfRef = useRef(false);

  usePersistedWindow("databaseEditor");

  useProjectSync();

  const windowActions = useCurrentWindowActions();

  // 首次有数据时选中 URL 指定或第一个 DataFrame；之后仅在当前选中被删除时回退
  useEffect(() => {
    const ids = Object.keys(dataframes);
    if (ids.length === 0) {
      hasInitializedDfRef.current = false;
      setSelectedDfId(null);
      return;
    }
    const dbFromUrl = getDatabaseIdFromUrl();
    const preferred = dbFromUrl && dataframes[dbFromUrl] ? dbFromUrl : ids[0];

    if (!hasInitializedDfRef.current) {
      hasInitializedDfRef.current = true;
      setSelectedDfId(preferred);
      return;
    }

    if (selectedDfId && !dataframes[selectedDfId]) {
      setSelectedDfId(ids[0] ?? null);
    }
  }, [dataframes, selectedDfId]);

  // 子窗口独立 WebView：先从后端同步项目，再展示窗口
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        await initializeProjectForCurrentWindow();
        if (cancelled) return;
        await windowActions.show();
      } catch (e) {
        reportViewIssue("app", e, "DatabaseEditorWindow");
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [windowActions]);

  const dfOptions: DataframeOption[] = useMemo(
    () =>
      Object.entries(dataframes).map(([id, df]) => ({
        label: df.name,
        value: id,
      })),
    [dataframes],
  );

  return (
    <div className="flex h-screen w-full flex-col overflow-hidden bg-background text-foreground font-sans">
      <DatabaseEditorContent
        databaseId={selectedDfId}
        refreshProjectOnRefresh
        renderHeader={(selectedCellText) => (
          <TitleBar
            dataframes={dfOptions}
            selectedDataframeId={selectedDfId}
            onSelectDataframe={setSelectedDfId}
            selectedCellText={selectedCellText}
          />
        )}
      />
    </div>
  );
};
