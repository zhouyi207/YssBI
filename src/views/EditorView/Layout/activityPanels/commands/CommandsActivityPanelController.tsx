import type { RootDockviewPanelComponent } from "../../RootDockviewHost";
import { SidebarCommandsTab } from "../../sidebar/tabs/SidebarCommandsTab";
import { ActivityPanelShell } from "../ActivityPanelShell";

function CommandsActivityPanelController() {
  return (
    <ActivityPanelShell>
      <SidebarCommandsTab />
    </ActivityPanelShell>
  );
}

export const commandsActivityPanelContribution: RootDockviewPanelComponent =
  CommandsActivityPanelController;
