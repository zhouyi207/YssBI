import { useCallback, useEffect, useMemo, useRef } from "react";
import { useTranslation } from "react-i18next";
import { useFunctionCatalog } from "@/features/core/editor";
import { useGraphSessionStore } from "@/features/core/graphSession/graphSessionStore";
import { workbenchDockviewRead } from "@/modules/workbench/public";
import { useProjectIOStore } from "@/features/application/project/projectIOStore";
import { useVariableStore } from "@/features/core/dataStore/variableStore";
import { useGraphResourcesByKind } from "@/features/core/resource";
import { useChartDocumentStore } from "@/features/core/chart/chartDocumentStore";
import { partitionVariableCatalog } from "@/features/core/variable/variableScopeSelectors";
import { useSidebarStore, type ProjectTreeCategoryId } from "@/features/core/sidebar";
import { buildProjectResourceBrowser, resolveActiveProjectGraph } from "./projectResourceBrowser";

export function useProjectResourceBrowser() {
  const { t } = useTranslation();
  const events = useGraphResourcesByKind("event");
  const functions = useFunctionCatalog();
  const variables = useVariableStore((state) => state.variables);
  const charts = useChartDocumentStore((state) => state.index);
  const projectInstanceId = useProjectIOStore((state) => state.projectInstanceId);
  const focusedSession = useGraphSessionStore((state) => state.focusedSession);
  const projectTreeQuery = useSidebarStore((state) => state.projectTreeQuery);
  const projectTreeExpandedCategories = useSidebarStore(
    (state) => state.projectTreeExpandedCategories,
  );
  const setProjectTreeQuery = useSidebarStore((state) => state.setProjectTreeQuery);
  const setProjectTreeCategoryExpanded = useSidebarStore(
    (state) => state.setProjectTreeCategoryExpanded,
  );
  const setProjectTreeCategoriesExpanded = useSidebarStore(
    (state) => state.setProjectTreeCategoriesExpanded,
  );
  const resetProjectTreeQuery = useSidebarStore((state) => state.resetProjectTreeQuery);
  const previousProjectInstanceId = useRef<string | null | undefined>(undefined);

  const activeEditor = focusedSession
    ? (workbenchDockviewRead.getActiveEditorPanelInGroup(focusedSession.groupId)?.metadata ?? null)
    : null;
  const activeGraph = useMemo(
    () => resolveActiveProjectGraph({ events, functions, activeEditor }),
    [activeEditor, events, functions],
  );
  const { global: globalVariables, local: localVariables } = useMemo(
    () =>
      partitionVariableCatalog(
        variables,
        activeGraph ? { graphPath: activeGraph.path, graphKind: activeGraph.kind } : undefined,
      ),
    [activeGraph, variables],
  );

  useEffect(() => {
    if (
      previousProjectInstanceId.current !== undefined &&
      previousProjectInstanceId.current !== projectInstanceId
    ) {
      resetProjectTreeQuery();
    }
    previousProjectInstanceId.current = projectInstanceId;
  }, [projectInstanceId, resetProjectTreeQuery]);

  const projection = useMemo(
    () =>
      buildProjectResourceBrowser({
        events,
        functions,
        charts,
        localVariables,
        globalVariables,
        activeGraph,
        query: projectTreeQuery,
        expandedCategoryIds: new Set(
          Object.entries(projectTreeExpandedCategories)
            .filter(([, expanded]) => expanded)
            .map(([categoryId]) => categoryId as ProjectTreeCategoryId),
        ),
        labels: {
          events: t("sidebar.projectTree.categories.events"),
          functions: t("sidebar.projectTree.categories.functions"),
          charts: t("sidebar.projectTree.categories.charts"),
          variables: t("sidebar.projectTree.categories.variables"),
          localVariables: t("sidebar.projectTree.categories.localVariables"),
          globalVariables: t("sidebar.projectTree.categories.globalVariables"),
          noEvents: t("sidebar.noEvents"),
          noFunctions: t("sidebar.noFunctions"),
          noCharts: t("chartsSidebar.noCharts"),
          noLocalVariables: t("sidebar.noLocalVariables"),
          noGlobalVariables: t("sidebar.noGlobalVariables"),
        },
      }),
    [
      activeGraph,
      events,
      functions,
      globalVariables,
      localVariables,
      projectTreeExpandedCategories,
      projectTreeQuery,
      t,
      charts,
    ],
  );

  const queryIsActive = projectTreeQuery.trim().length > 0;
  const setCategoryExpanded = useCallback(
    (categoryId: ProjectTreeCategoryId, expanded: boolean) => {
      if (queryIsActive) return;
      setProjectTreeCategoryExpanded(categoryId, expanded);
    },
    [queryIsActive, setProjectTreeCategoryExpanded],
  );
  const toggleAllCategories = useCallback(() => {
    if (!projection.canToggleAllCategories) return;
    setProjectTreeCategoriesExpanded(projection.categoryIds, !projection.allCategoriesExpanded);
  }, [projection, setProjectTreeCategoriesExpanded]);

  return {
    ...projection,
    query: projectTreeQuery,
    queryIsActive,
    activeGraph,
    setQuery: setProjectTreeQuery,
    resetQuery: resetProjectTreeQuery,
    setCategoryExpanded,
    toggleAllCategories,
  };
}
