import { useTranslation } from 'react-i18next';
import { useCallFunctionIssueCountsByGraph } from '@/features/application/graphDiagnostics/useCallFunctionDiagnostics';
import { useProjectResourceBrowser } from '@/features/application/sidebar/useProjectResourceBrowser';
import { useDetailTarget } from '@/features/application/editor';
import {
  SidebarTreeSearchInput,
  SidebarVirtualTree,
  sidebarTreeSearchShellClass,
} from '../../sidebarUi';
import { SidebarProjectTreeRow, type SidebarProjectTreeActions } from '../rows/SidebarProjectTreeRow';
import { SidebarTabPanel } from '../sections/SidebarTabPanel';

const PROJECT_TREE_ROW_HEIGHT = 28;

export function SidebarProjectTab({
  actions,
}: {
  actions: SidebarProjectTreeActions;
}) {
  const { t } = useTranslation();
  const detailTarget = useDetailTarget();
  const graphIssueCounts = useCallFunctionIssueCountsByGraph();
  const {
    rows,
    query,
    queryIsActive,
    allCategoriesExpanded,
    canToggleAllCategories,
    setQuery,
    setCategoryExpanded,
    toggleAllCategories,
  } = useProjectResourceBrowser();

  return (
    <SidebarTabPanel>
      <div className={sidebarTreeSearchShellClass()}>
        <SidebarTreeSearchInput
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          placeholder={t('sidebar.projectTree.searchPlaceholder')}
          expandAllLabel={t('sidebar.projectTree.expandAll')}
          collapseAllLabel={t('sidebar.projectTree.collapseAll')}
          allCategoriesExpanded={allCategoriesExpanded}
          canToggleAllCategories={canToggleAllCategories}
          onToggleAllCategories={toggleAllCategories}
        />
      </div>
      <SidebarVirtualTree
        rows={rows}
        ariaLabel={t('activityBar.project')}
        emptyMessage={t('sidebar.projectTree.noMatches')}
        getRowKey={(row) => row.rowKey}
        getRowDepth={(row) => row.level}
        estimateSize={() => PROJECT_TREE_ROW_HEIGHT}
        renderRow={(row) => (
          <SidebarProjectTreeRow
            row={row}
            actions={actions}
            detailTarget={detailTarget}
            graphIssueCounts={graphIssueCounts}
            categoryInteractionDisabled={queryIsActive}
            onCategoryExpandedChange={setCategoryExpanded}
          />
        )}
      />
    </SidebarTabPanel>
  );
}
