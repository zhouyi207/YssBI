import type { LogDomainId } from "@/features/application/log";
import { LogPanelStatus } from "./LogPanelStatus";
import { LogPanelToolbar } from "./LogPanelToolbar";

export function LogWorkspaceActions({ domain }: { readonly domain?: LogDomainId }) {
  return (
    <div
      data-yssbi-logs-header-actions
      className="flex h-full shrink-0 items-center gap-1 px-1"
      onPointerDown={(event) => event.stopPropagation()}
      onMouseDown={(event) => event.stopPropagation()}
    >
      {domain ? <LogPanelStatus domain={domain} /> : null}
      {domain ? <LogPanelToolbar /> : null}
    </div>
  );
}
