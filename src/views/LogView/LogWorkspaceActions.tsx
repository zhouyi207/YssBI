import { useCallback, useSyncExternalStore } from 'react';
import { useTranslation } from 'react-i18next';
import { VscAdd } from 'react-icons/vsc';
import type {
  DockviewApi,
  DockviewGroupPanel,
  IDockviewHeaderActionsProps,
  IDockviewPanel,
  IWatermarkPanelProps,
} from 'dockview-react';

import { Button } from '@/components/ui/button';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import {
  LOGS_DOCKVIEW_COMPONENT_ID,
  type LogsDockviewPanelParams,
} from '@/features/core/dockview/logsDockviewLayout';
import {
  isLogDomainId,
  LOG_DOMAIN_ORDER,
  logDomainPanelId,
  logDomainTitle,
  type LogDomainId,
} from '@/features/core/log/logDomains';
import { applyLogFilter } from '@/features/core/log/logStore';
import { LogPanelStatus } from './LogPanelStatus';
import { LogPanelToolbar } from './LogPanelToolbar';
import { LOG_DOMAIN_TITLE_KEYS } from './logPresentation';
import { useLogWorkspaceContext } from './logWorkspaceContext';

function closedDomainMask(api: DockviewApi): number {
  return LOG_DOMAIN_ORDER.reduce((mask, domain, index) => (
    api.getPanel(logDomainPanelId(domain)) ? mask : mask | (1 << index)
  ), 0);
}

function useClosedLogDomains(api: DockviewApi): readonly LogDomainId[] {
  const subscribe = useCallback((listener: () => void) => {
    const disposable = api.onDidLayoutChange(listener);
    return () => disposable.dispose();
  }, [api]);
  const getSnapshot = useCallback(() => closedDomainMask(api), [api]);
  const mask = useSyncExternalStore(subscribe, getSnapshot, getSnapshot);

  return LOG_DOMAIN_ORDER.filter((_, index) => (mask & (1 << index)) !== 0);
}

function panelDomain(panel: IDockviewPanel | undefined): LogDomainId | undefined {
  const domain = panel?.params?.domain;
  return isLogDomainId(domain) ? domain : undefined;
}

function resolveTargetGroup(
  api: DockviewApi,
  preferredGroupId: string | undefined,
): DockviewGroupPanel {
  const preferred = preferredGroupId
    ? api.groups.find((group) => group.id === preferredGroupId)
    : undefined;
  return preferred ?? api.activeGroup ?? api.groups[0] ?? api.addGroup();
}

function openLogDomain(
  api: DockviewApi,
  preferredGroupId: string | undefined,
  domain: LogDomainId,
): void {
  const targetGroup = resolveTargetGroup(api, preferredGroupId);
  const id = logDomainPanelId(domain);
  const existing = api.getPanel(id);
  if (existing) {
    if (existing.group.id !== targetGroup.id) {
      existing.api.moveTo({ group: targetGroup });
    }
    existing.api.setActive();
    return;
  }

  const panel = api.addPanel<LogsDockviewPanelParams>({
    id,
    component: LOGS_DOCKVIEW_COMPONENT_ID,
    params: { domain },
    title: logDomainTitle(domain),
    position: { referenceGroup: targetGroup.id, direction: 'within' },
  });
  panel.api.setActive();
}

interface ClosedLogDomainMenuProps {
  readonly api: DockviewApi;
  readonly preferredGroupId?: string;
  readonly presentation: 'header' | 'watermark';
}

function ClosedLogDomainMenu({
  api,
  preferredGroupId,
  presentation,
}: ClosedLogDomainMenuProps) {
  const { t } = useTranslation();
  const closedDomains = useClosedLogDomains(api);
  if (closedDomains.length === 0) return null;

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button
          type="button"
          variant={presentation === 'header' ? 'ghost' : 'outline'}
          size={presentation === 'header' ? 'icon-sm' : 'sm'}
          aria-label={t('log.openDomain')}
        >
          <VscAdd data-icon="inline-start" />
          {presentation === 'watermark' ? t('log.openDomain') : null}
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent
        align={presentation === 'header' ? 'end' : 'center'}
        className="min-w-32 rounded-sm p-0"
      >
        <DropdownMenuGroup>
          {closedDomains.map((domain) => (
            <DropdownMenuItem
              key={domain}
              className="rounded-sm px-2 py-1 text-xs"
              onSelect={() => openLogDomain(api, preferredGroupId, domain)}
            >
              {t(LOG_DOMAIN_TITLE_KEYS[domain])}
            </DropdownMenuItem>
          ))}
        </DropdownMenuGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  );
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
      className="flex h-full shrink-0 items-center gap-1 px-1"
      onPointerDown={stopHeaderControlPropagation}
      onMouseDown={stopHeaderControlPropagation}
    >
      {domain ? <LogPanelStatus filteredLogCount={filteredLogCount} /> : null}
      {domain ? <LogPanelToolbar /> : null}
      <ClosedLogDomainMenu
        api={props.containerApi}
        preferredGroupId={props.group.id}
        presentation="header"
      />
    </div>
  );
}

export function LogWorkspaceWatermark(props: IWatermarkPanelProps) {
  return (
    <div
      className="flex h-full min-h-0 items-center justify-center"
      onPointerDown={stopHeaderControlPropagation}
      onMouseDown={stopHeaderControlPropagation}
    >
      <ClosedLogDomainMenu
        api={props.containerApi}
        preferredGroupId={props.group?.id}
        presentation="watermark"
      />
    </div>
  );
}
