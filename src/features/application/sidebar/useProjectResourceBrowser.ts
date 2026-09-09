import { useMemo } from "react";
import { useTranslation } from "react-i18next";
import { useFunctionCatalog } from "@/features/core/editor";
import { useGraphSessionStore } from "@/features/core/graphSession/graphSessionStore";
import { workbenchDockviewRead } from "@/modules/workbench/public";
import { useGraphResourcesByKind } from "@/features/core/resource";
import { useChartDocumentStore } from "@/features/core/chart/chartDocumentStore";
import { useDatabaseRead } from "@/features/core/database/read";
import { useSidebarStore, type ProjectTreeCategoryId } from "@/features/core/sidebar";
import { buildProjectResourceBrowser, resolveActiveProjectGraph } from "./projectResourceBrowser";

export function useProjectResourceBrowser() {
  const { t } = useTranslation();
  const events = useGraphResourcesByKind("event");
  const functions = useFunctionCatalog();
  const charts = useChartDocumentStore((state) => state.index);
  const databases = useDatabaseRead((snapshot) => snapshot.databases);
  const focusedSession = useGraphSessionStore((state) => state.focusedSession);
  const projectTreeExpandedCategories = useSidebarStore(
    (state) => state.projectTreeExpandedCategories,
  );
  const setProjectTreeCategoryExpanded = useSidebarStore(
    (state) => state.setProjectTreeCategoryExpanded,
  );

  const activeEditor = focusedSession
    ? (workbenchDockviewRead.getActiveEditorPanelInGroup(focusedSession.groupId)?.metadata ?? null)
    : null;
  const activeGraph = useMemo(
    () => resolveActiveProjectGraph({ events, functions, activeEditor }),
    [activeEditor, events, functions],
  );

  const projection = useMemo(
    () =>
      buildProjectResourceBrowser({
        events,
        functions,
        charts,
        databases,
        expandedCategoryIds: new Set(
          Object.entries(projectTreeExpandedCategories)
            .filter(([, expanded]) => expanded)
            .map(([categoryId]) => categoryId as ProjectTreeCategoryId),
        ),
        labels: {
          events: t("sidebar.projectTree.categories.events"),
          functions: t("sidebar.projectTree.categories.functions"),
          charts: t("sidebar.projectTree.categories.charts"),
          data: t("sidebar.sections.data"),
          noEvents: t("sidebar.noEvents"),
          noFunctions: t("sidebar.noFunctions"),
          noCharts: t("chartsSidebar.noCharts"),
          noData: t("sidebar.noData"),
        },
      }),
    [events, functions, projectTreeExpandedCategories, t, charts, databases],
  );

  return {
    ...projection,
    activeGraph,
    setCategoryExpanded: setProjectTreeCategoryExpanded,
  };
}
