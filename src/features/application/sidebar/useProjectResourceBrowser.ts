import { useCallback, useEffect, useMemo, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { useFunctionCatalog } from '@/features/core/editor';
import { editorDockviewPort, useDockviewPortSnapshot } from '@/features/core/dockview';
import { getActiveLayoutTab } from '@/features/core/layout/layoutTabQueries';
import { useProjectIOStore, useVariableStore } from '@/features/core/dataStore';
import { useGraphResourcesByKind } from '@/features/core/resource';
import { useWorksheetStore } from '@/features/core/worksheet/worksheetStore';
import { partitionVariableCatalog } from '@/features/core/variable/variableScopeSelectors';
import {
  useSidebarStore,
  type ProjectTreeCategoryId,
} from '@/features/core/sidebar';
import {
  buildProjectResourceBrowser,
  resolveActiveProjectGraph,
} from './projectResourceBrowser';

export function useProjectResourceBrowser() {
  const { t } = useTranslation();
  const events = useGraphResourcesByKind('event');
  const functions = useFunctionCatalog();
  const variables = useVariableStore((state) => state.variables);
  const worksheets = useWorksheetStore((state) => state.index);
  const projectInstanceId = useProjectIOStore((state) => state.projectInstanceId);
  useDockviewPortSnapshot(editorDockviewPort);
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

  const activeGroupId = editorDockviewPort.getActiveGroupId() ?? null;
  const activeTab = activeGroupId ? getActiveLayoutTab(activeGroupId)?.tab ?? null : null;
  const activeGraph = useMemo(
    () => resolveActiveProjectGraph({ events, functions, activeTab }),
    [activeTab, events, functions],
  );
  const { global: globalVariables, local: localVariables } = useMemo(
    () => partitionVariableCatalog(
      variables,
      activeGraph
        ? { graphPath: activeGraph.path, graphKind: activeGraph.kind }
        : undefined,
    ),
    [activeGraph, variables],
  );

  useEffect(() => {
    if (previousProjectInstanceId.current !== undefined
      && previousProjectInstanceId.current !== projectInstanceId) {
      resetProjectTreeQuery();
    }
    previousProjectInstanceId.current = projectInstanceId;
  }, [projectInstanceId, resetProjectTreeQuery]);

  const projection = useMemo(() => buildProjectResourceBrowser({
    events,
    functions,
    worksheets,
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
      events: t('sidebar.projectTree.categories.events'),
      functions: t('sidebar.projectTree.categories.functions'),
      worksheets: t('sidebar.projectTree.categories.worksheets'),
      activeGraphVariables: (graphName) => t(
        'sidebar.projectTree.categories.activeGraphVariables',
        { name: graphName },
      ),
      globalVariables: t('sidebar.projectTree.categories.globalVariables'),
      noEvents: t('sidebar.noEvents'),
      noFunctions: t('sidebar.noFunctions'),
      noWorksheets: t('chartsSidebar.noWorksheets'),
      noLocalVariables: t('sidebar.noLocalVariables'),
      noGlobalVariables: t('sidebar.noGlobalVariables'),
    },
  }), [
    activeGraph,
    events,
    functions,
    globalVariables,
    localVariables,
    projectTreeExpandedCategories,
    projectTreeQuery,
    t,
    worksheets,
  ]);

  const queryIsActive = projectTreeQuery.trim().length > 0;
  const setCategoryExpanded = useCallback((categoryId: ProjectTreeCategoryId, expanded: boolean) => {
    if (queryIsActive) return;
    setProjectTreeCategoryExpanded(categoryId, expanded);
  }, [queryIsActive, setProjectTreeCategoryExpanded]);
  const toggleAllCategories = useCallback(() => {
    if (!projection.canToggleAllCategories) return;
    setProjectTreeCategoriesExpanded(
      projection.categoryIds,
      !projection.allCategoriesExpanded,
    );
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
