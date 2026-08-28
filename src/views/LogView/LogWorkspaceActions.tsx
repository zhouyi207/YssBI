import type {
  IDockviewHeaderActionsProps,
  IDockviewPanel,
} from 'dockview-react';

import { isLogDomainId, applyLogFilter, type LogDomainId } from '@/features/application/viewCapabilities';
import { LogPanelStatus } from './LogPanelStatus';
import { LogPanelToolbar } from './LogPanelToolbar';
import { useLogWorkspaceContext } from './logWorkspaceContext';

function panelDomain(panel: IDockviewPanel | undefined): LogDomainId | undefined {
  const domain = panel?.params?.domain;
  return isLogDomainId(domain) ? domain : undefined;
}

function stopHeaderControlPropagation(event: { stopPropagation(): void }): void {
  event.stopPropagation();
}

export function LogWorkspaceActions(props: IDockviewHeaderActionsProps) {
  const { logs, filter } = useLogWorkspaceContext();
  const domain = panelDomain(props.activePanel);
  const filteredLogCount = domain
    ? applyLogFilter(logs, filter, domain).length
    : 0;

  return (
    <div
      data-yssbi-logs-header-actions
      className="flex h-full shrink-0 items-center gap-1 px-1"
      onPointerDown={stopHeaderControlPropagation}
      onMouseDown={stopHeaderControlPropagation}
    >
      {domain ? <LogPanelStatus filteredLogCount={filteredLogCount} /> : null}
      {domain ? <LogPanelToolbar /> : null}
    </div>
  );
}
