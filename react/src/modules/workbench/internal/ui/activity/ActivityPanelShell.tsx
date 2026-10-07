import type { ReactNode } from "react";

export function ActivityPanelShell({
  children,
  title,
  tools,
}: {
  readonly children: ReactNode;
  readonly title: string;
  readonly tools?: ReactNode;
}) {
  return (
    <div
      className="sidebar-container relative z-30 flex h-full w-full min-w-0 select-none overflow-hidden bg-sidebar"
      style={{ pointerEvents: "auto" }}
      data-workbench-activity-panel
    >
      <div className="flex min-h-0 min-w-0 flex-1 flex-col bg-sidebar">
        <header
          className="flex h-[var(--workbench-tab-height)] shrink-0 items-center gap-2 px-3 text-xs"
          data-workbench-activity-panel-header
        >
          <h2 className="min-w-0 flex-1 truncate font-normal uppercase">{title}</h2>
          {tools}
        </header>
        <div className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden p-0">{children}</div>
      </div>
    </div>
  );
}
