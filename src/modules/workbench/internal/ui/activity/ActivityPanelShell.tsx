import type { ReactNode } from "react";

export function ActivityPanelShell({ children }: { readonly children: ReactNode }) {
  return (
    <div
      className="sidebar-container relative z-30 flex h-full w-full min-w-0 select-none overflow-hidden bg-sidebar"
      style={{ pointerEvents: "auto" }}
      data-workbench-activity-panel
    >
      <div className="flex min-h-0 min-w-0 flex-1 flex-col bg-sidebar">
        <div className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden p-0">{children}</div>
      </div>
    </div>
  );
}
