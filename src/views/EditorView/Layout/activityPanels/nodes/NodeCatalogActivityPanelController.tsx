import type { RootDockviewPanelComponent } from "../../RootDockviewHost";
import { SidebarNodesTab } from "../../sidebar/tabs/SidebarNodesTab";
import { ActivityPanelShell } from "../ActivityPanelShell";

function NodeCatalogActivityPanelController() {
  return (
    <ActivityPanelShell>
      <SidebarNodesTab />
    </ActivityPanelShell>
  );
}

export const nodeCatalogActivityPanelContribution: RootDockviewPanelComponent =
  NodeCatalogActivityPanelController;
