import { useCallback, useEffect, useRef } from 'react';
import {
  DockviewReact,
  type DockviewApi,
  type DockviewReadyEvent,
  type DockviewWillDropEvent,
  type IDockviewPanelHeaderProps,
} from 'dockview-react';

import {
  DEFAULT_LOGS_DOCKVIEW_LAYOUT,
  LOGS_DOCKVIEW_COMPONENT_ID,
} from '@/features/core/dockview/logsDockviewLayout';
import type { LogsDockviewLayoutController } from '@/features/core/dockview/logsDockviewLayoutController';
import { useSettingsStore } from '@/features/core/settings/settingsStore';
import { resolveYssbiLogsDockviewTheme } from '@/shared/theme/dockviewTheme';
import { LogDomainPanel } from './LogDomainPanel';
import { LogWorkspaceActions } from './LogWorkspaceActions';
import { LogWorkspaceProvider } from './logWorkspaceContext';

const LOG_WORKSPACE_COMPONENTS = {
  [LOGS_DOCKVIEW_COMPONENT_ID]: LogDomainPanel,
};

export type LogWorkspaceLayoutLifecycle =
  | { readonly kind: 'main'; readonly controller: LogsDockviewLayoutController }
  | { readonly kind: 'ephemeral' };

export interface LogWorkspaceDockviewProps {
  readonly layout: LogWorkspaceLayoutLifecycle;
}

interface BoundMainLayout {
  readonly api: DockviewApi;
  readonly controller: LogsDockviewLayoutController;
}

function LogWorkspaceTab({ api }: IDockviewPanelHeaderProps) {
  return (
    <div className="dv-default-tab yssbi-logs-tab" data-yssbi-logs-tab>
      <span className="dv-default-tab-content">{api.title}</span>
    </div>
  );
}

function restrictLogsTabDrop(event: DockviewWillDropEvent): void {
  const transfer = event.getData();
  const isSameGroupTabDrop = event.kind === 'tab'
    && transfer?.panelId !== null
    && transfer?.panelId !== undefined
    && transfer.groupId === event.group?.id;

  if (!isSameGroupTabDrop) event.preventDefault();
}

export function LogWorkspaceDockview({ layout }: LogWorkspaceDockviewProps) {
  const boundMainLayoutRef = useRef<BoundMainLayout | null>(null);
  const themeMode = useSettingsStore((state) => state.theme.mode);
  const mainController = layout.kind === 'main' ? layout.controller : null;
  const presentation = layout.kind === 'main' ? 'embedded' : 'standalone';

  const onReady = useCallback((event: DockviewReadyEvent) => {
    if (mainController) {
      mainController.bind(event.api);
      boundMainLayoutRef.current = { api: event.api, controller: mainController };
      return;
    }

    event.api.fromJSON(structuredClone(DEFAULT_LOGS_DOCKVIEW_LAYOUT));
  }, [mainController]);

  useEffect(() => () => {
    const bound = boundMainLayoutRef.current;
    boundMainLayoutRef.current = null;
    if (bound) bound.controller.unbind(bound.api);
  }, []);

  return (
    <LogWorkspaceProvider presentation={presentation}>
      <div data-yssbi-logs-dockview className="h-full min-h-0 w-full min-w-0">
        <DockviewReact
          className="yssbi-logs-dockview-instance h-full w-full"
          components={LOG_WORKSPACE_COMPONENTS}
          rightHeaderActionsComponent={LogWorkspaceActions}
          defaultTabComponent={LogWorkspaceTab}
          disableFloatingGroups
          theme={resolveYssbiLogsDockviewTheme(themeMode)}
          onWillDrop={restrictLogsTabDrop}
          onReady={onReady}
        />
      </div>
    </LogWorkspaceProvider>
  );
}
