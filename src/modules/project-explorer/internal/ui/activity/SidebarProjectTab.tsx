import { useTranslation } from "react-i18next";
import { useGraphDiagnosticCounts } from "@/features/application/graphDiagnostics/useGraphDiagnosticCounts";
import { useProjectResourceBrowser } from "@/features/application/sidebar/useProjectResourceBrowser";
import { useDetailTarget } from "@/features/application/editor";
import { SidebarTabPanel, SidebarVirtualTree } from "@/modules/workbench/public";
import { SidebarProjectTreeRow, type SidebarProjectTreeActions } from "./SidebarProjectTreeRow";

const PROJECT_TREE_ROW_HEIGHT = 28;

export function SidebarProjectTab({ actions }: { actions: SidebarProjectTreeActions }) {
  const { t } = useTranslation();
  const detailTarget = useDetailTarget();
  const graphDiagnosticCounts = useGraphDiagnosticCounts();
  const { rows, setCategoryExpanded } = useProjectResourceBrowser();

  return (
    <SidebarTabPanel>
      <SidebarVirtualTree
        rows={rows}
        ariaLabel={t("activityBar.project")}
        emptyMessage={t("sidebar.projectTree.empty")}
        getRowKey={(row) => row.rowKey}
        getRowDepth={(row) => row.level}
        estimateSize={() => PROJECT_TREE_ROW_HEIGHT}
        renderRow={(row) => (
          <SidebarProjectTreeRow
            row={row}
            actions={actions}
            detailTarget={detailTarget}
            graphDiagnosticCounts={graphDiagnosticCounts}
            onCategoryExpandedChange={setCategoryExpanded}
          />
        )}
      />
    </SidebarTabPanel>
  );
}
