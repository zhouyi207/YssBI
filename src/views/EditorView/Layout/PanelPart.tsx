import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  type ComponentType,
  type KeyboardEvent,
} from 'react';
import { useTranslation } from 'react-i18next';
import { FiChevronDown, FiChevronUp } from 'react-icons/fi';
import {
  DockviewDefaultTab,
  DockviewReact,
  type DockviewApi,
  type DockviewReadyEvent,
  type IDockviewHeaderActionsProps,
  type IDockviewPanelHeaderProps,
  type IDockviewPanelProps,
} from 'dockview-react';

import {
  panelDockviewPort,
  useDockviewPortSnapshot,
} from '@/features/core/dockview';
import {
  getPanelViewLabelKey,
  PANEL_VIEW_IDS,
  PANEL_VIEW_SPECS,
  type PanelViewId,
} from '@/features/core/layout/panelPartModel';
import { togglePanelCollapsed } from '@/features/core/layout/workbenchLayoutService';
import {
  DEFAULT_WORKBENCH_PANEL_SIZE,
  WORKBENCH_PANEL_COLLAPSED_HEIGHT,
} from '@/features/core/workbench';
import { ToolbarIconButton } from '@/shared/ui/ToolbarIconButton';
import { LogPanelProvider } from '@/views/LogView/logPanelContext';
import { LogPanel } from '@/views/LogView/LogPanel';
import { OutputPanel } from '@/views/LogView/OutputPanel';

const WORKBENCH_EDITOR_HOST_PANEL_ID = 'workbench-editor-host';
const WORKBENCH_PANEL_EDGE_GROUP_ID = 'workbench-panel-bottom';

interface PanelPartProps {
  editorComponent: ComponentType;
}

function isPanelViewId(value: string): value is PanelViewId {
  return (PANEL_VIEW_IDS as readonly string[]).includes(value);
}

function EmbeddedLogPanel(_: IDockviewPanelProps) {
  return (
    <LogPanelProvider variant="embedded">
      <LogPanel />
    </LogPanelProvider>
  );
}

function PanelDockviewTab(props: IDockviewPanelHeaderProps) {
  const { t } = useTranslation();
  const viewId = isPanelViewId(props.api.id) ? props.api.id : null;
  const title = viewId ? t(getPanelViewLabelKey(viewId)) : props.api.title;

  useEffect(() => {
    if (title && props.api.title !== title) props.api.setTitle(title);
  }, [props.api, title]);

  return <DockviewDefaultTab {...props} hideClose />;
}

function PanelDockviewActions(_: IDockviewHeaderActionsProps) {
  const { t } = useTranslation();
  const panelCollapsed = useDockviewPortSnapshot(panelDockviewPort).collapsed ?? false;

  return (
    <div className="flex h-full items-center px-1">
      <ToolbarIconButton
        type="button"
        variant="ghost"
        size="icon-sm"
        onClick={togglePanelCollapsed}
        tooltip={t(panelCollapsed ? 'log.expandPanel' : 'log.collapsePanel')}
        aria-label={t(panelCollapsed ? 'log.expandPanel' : 'log.collapsePanel')}
        aria-expanded={!panelCollapsed}
      >
        {panelCollapsed
          ? <FiChevronUp data-icon="inline-start" />
          : <FiChevronDown data-icon="inline-start" />}
      </ToolbarIconButton>
    </div>
  );
}

function initializePanelDock(
  api: DockviewApi,
  titleFor: (viewId: PanelViewId) => string,
): void {
  const editorHost = api.addPanel({
    id: WORKBENCH_EDITOR_HOST_PANEL_ID,
    component: 'EditorHost',
  });
  editorHost.group.header.hidden = true;
  editorHost.group.locked = 'no-drop-target';

  const panelGroup = api.addEdgeGroup('bottom', {
    id: WORKBENCH_PANEL_EDGE_GROUP_ID,
    initialSize: DEFAULT_WORKBENCH_PANEL_SIZE,
    minimumSize: WORKBENCH_PANEL_COLLAPSED_HEIGHT,
    collapsedSize: WORKBENCH_PANEL_COLLAPSED_HEIGHT,
    collapsed: false,
  });
  const logs = api.addPanel({
    id: 'logs',
    component: PANEL_VIEW_SPECS.logs.component,
    title: titleFor('logs'),
    position: { referenceGroup: panelGroup.id, direction: 'within' },
  });
  api.addPanel({
    id: 'output',
    component: PANEL_VIEW_SPECS.output.component,
    title: titleFor('output'),
    inactive: true,
    position: { referencePanel: logs.id, direction: 'within' },
  });
  editorHost.api.setActive();
}

function preventFixedPanelClose(event: KeyboardEvent<HTMLDivElement>): void {
  if (event.key !== 'Delete' && event.key !== 'Backspace') return;
  if (!(event.target instanceof Element) || !event.target.closest('.dv-tab')) return;
  event.preventDefault();
  event.stopPropagation();
}

export function PanelPart({ editorComponent: EditorComponent }: PanelPartProps) {
  const { t } = useTranslation();
  const apiRef = useRef<DockviewApi | null>(null);
  const shellComponents = useMemo(() => ({
    EditorHost: function EditorHost() {
      return <EditorComponent />;
    },
    LogPanel: EmbeddedLogPanel,
    OutputPanel,
  }), [EditorComponent]);

  const onReady = useCallback((event: DockviewReadyEvent) => {
    apiRef.current = event.api;
    initializePanelDock(
      event.api,
      (viewId) => t(getPanelViewLabelKey(viewId)),
    );
    panelDockviewPort.bind(event.api);
  }, [t]);

  useEffect(() => () => {
    const boundApi = apiRef.current;
    apiRef.current = null;
    if (boundApi) panelDockviewPort.unbind(boundApi);
  }, []);

  return (
    <div
      className="h-full min-h-0 w-full min-w-0 overflow-hidden"
      data-yssbi-panel-dock
      onKeyDownCapture={preventFixedPanelClose}
    >
      <DockviewReact
        className="dockview-theme-dark h-full w-full"
        components={shellComponents}
        defaultTabComponent={PanelDockviewTab}
        rightHeaderActionsComponent={PanelDockviewActions}
        disableFloatingGroups
        onReady={onReady}
      />
    </div>
  );
}
