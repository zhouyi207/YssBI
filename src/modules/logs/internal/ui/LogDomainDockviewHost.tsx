import { useCallback, useEffect, useRef, useState } from "react";
import {
  DockviewReact,
  type DockviewReadyEvent,
  type DockviewWillDropEvent,
  type IDockviewPanelHeaderProps,
} from "dockview-react";

import {
  DEFAULT_LOGS_DOCKVIEW_LAYOUT,
  LOGS_DOCKVIEW_COMPONENT_ID,
} from "@/features/core/dockview/logsDockviewLayout";
import { logsDockviewRootBinding } from "@/features/core/dockview";
import type { LogsDockviewBindingToken } from "@/features/core/dockview";
import { useSettingsRead } from "@/features/core/settings/read";
import { resolveYssbiLogsDockviewTheme } from "@/shared/theme/dockviewTheme";
import { LogDomainPanel } from "./LogDomainPanel";
import { LogWorkspaceActions } from "./LogWorkspaceActions";
import { LogWorkspaceProvider } from "./logWorkspaceContext";

const LOG_DOMAIN_COMPONENTS = {
  [LOGS_DOCKVIEW_COMPONENT_ID]: LogDomainPanel,
};

export type LogDomainLayoutLifecycle = { readonly kind: "main" } | { readonly kind: "ephemeral" };

export interface LogDomainDockviewHostProps {
  readonly layout: LogDomainLayoutLifecycle;
}

interface BoundMainLayout {
  readonly token: LogsDockviewBindingToken;
}

function LogDomainTab({ api }: IDockviewPanelHeaderProps) {
  const [title, setTitle] = useState(api.title);

  useEffect(() => {
    const disposable = api.onDidTitleChange((event) => setTitle(event.title));
    setTitle(api.title);
    return () => disposable.dispose();
  }, [api]);

  return (
    <div className="dv-default-tab yssbi-logs-tab" data-yssbi-logs-tab>
      <span className="dv-default-tab-content">{title}</span>
    </div>
  );
}

function restrictLogsTabDrop(event: DockviewWillDropEvent): void {
  const transfer = event.getData();
  const isSameGroupTabDrop =
    event.kind === "tab" &&
    transfer?.panelId !== null &&
    transfer?.panelId !== undefined &&
    transfer.groupId === event.group?.id;

  if (!isSameGroupTabDrop) event.preventDefault();
}

export function LogDomainDockviewHost({ layout }: LogDomainDockviewHostProps) {
  const boundMainLayoutRef = useRef<BoundMainLayout | null>(null);
  const themeMode = useSettingsRead((state) => state.theme.mode);
  const isMainLayout = layout.kind === "main";
  const presentation = isMainLayout ? "embedded" : "standalone";

  const onReady = useCallback(
    (event: DockviewReadyEvent) => {
      if (isMainLayout) {
        const token = logsDockviewRootBinding.bind(event.api);
        boundMainLayoutRef.current = { token };
        return;
      }

      event.api.fromJSON(structuredClone(DEFAULT_LOGS_DOCKVIEW_LAYOUT));
    },
    [isMainLayout],
  );

  useEffect(
    () => () => {
      const bound = boundMainLayoutRef.current;
      boundMainLayoutRef.current = null;
      if (bound) logsDockviewRootBinding.unbind(bound.token);
    },
    [],
  );

  return (
    <LogWorkspaceProvider presentation={presentation}>
      <div data-yssbi-logs-dockview className="h-full min-h-0 w-full min-w-0">
        <DockviewReact
          className="yssbi-logs-dockview-instance h-full w-full"
          components={LOG_DOMAIN_COMPONENTS}
          rightHeaderActionsComponent={LogWorkspaceActions}
          defaultTabComponent={LogDomainTab}
          disableFloatingGroups
          theme={resolveYssbiLogsDockviewTheme(themeMode)}
          onWillDrop={restrictLogsTabDrop}
          onReady={onReady}
        />
      </div>
    </LogWorkspaceProvider>
  );
}
