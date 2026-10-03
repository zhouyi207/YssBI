import React, { useEffect, useState, useMemo, useRef } from "react";
import { useShallow } from "zustand/react/shallow";
import { useProjectSync } from "@/features/application/initialization";
import { initializeProjectForCurrentWindow } from "@/features/application/project";
import { useCurrentWindowActions } from "@/features/application/window";
import { selectDatabaseNames, useDatabaseRead } from "@/features/core/database/read";
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
  const databaseNames = useDatabaseRead(useShallow(selectDatabaseNames));

  const [selectedDfId, setSelectedDfId] = useState<string | null>(null);
  const hasInitializedDfRef = useRef(false);

  useProjectSync();

  const { show } = useCurrentWindowActions();

  // 首次有数据时选中 URL 指定或第一个 DataFrame；之后仅在当前选中被删除时回退
  useEffect(() => {
    const ids = Object.keys(databaseNames);
    if (ids.length === 0) {
      hasInitializedDfRef.current = false;
      setSelectedDfId(null);
      return;
    }
    const dbFromUrl = getDatabaseIdFromUrl();
    const preferred =
      dbFromUrl && Object.prototype.hasOwnProperty.call(databaseNames, dbFromUrl)
        ? dbFromUrl
        : ids[0];

    if (!hasInitializedDfRef.current) {
      hasInitializedDfRef.current = true;
      setSelectedDfId(preferred);
      return;
    }

    if (selectedDfId && !Object.prototype.hasOwnProperty.call(databaseNames, selectedDfId)) {
      setSelectedDfId(ids[0] ?? null);
    }
  }, [databaseNames, selectedDfId]);

  // 子窗口独立 WebView：先从后端同步项目，再展示窗口
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        await initializeProjectForCurrentWindow();
        if (cancelled) return;
        await show();
      } catch (e) {
        reportViewIssue("app", e, "DatabaseEditorWindow");
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [show]);

  const dfOptions: DataframeOption[] = useMemo(
    () =>
      Object.entries(databaseNames).map(([id, name]) => ({
        label: name,
        value: id,
      })),
    [databaseNames],
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
