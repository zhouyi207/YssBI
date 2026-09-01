import { ActivityPanelShell, type RootDockviewPanelComponent } from "@/modules/workbench/public";
import { SidebarCommandsTab } from "./SidebarCommandsTab";

function CommandsActivityPanelController() {
  return (
    <ActivityPanelShell>
      <SidebarCommandsTab />
    </ActivityPanelShell>
  );
}

export const commandsActivityPanelContribution: RootDockviewPanelComponent =
  CommandsActivityPanelController;
