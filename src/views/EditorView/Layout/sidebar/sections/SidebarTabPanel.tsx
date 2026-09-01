import type { ReactNode } from "react";

/** Standard flex shell for every sidebar tab panel. */
export function SidebarTabPanel({ children, footer }: { children: ReactNode; footer?: ReactNode }) {
  return (
    <div className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden">
      <div className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden">{children}</div>
      {footer ? <div className="shrink-0">{footer}</div> : null}
    </div>
  );
}
