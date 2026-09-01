import { ActivityPanelShell, type RootDockviewPanelComponent } from "@/modules/workbench/public";
import { SidebarNodesTab } from "./SidebarNodesTab";

function NodeCatalogActivityPanelController() {
  return (
    <ActivityPanelShell>
      <SidebarNodesTab />
    </ActivityPanelShell>
  );
}

export const nodeCatalogActivityPanelContribution: RootDockviewPanelComponent =
  NodeCatalogActivityPanelController;
