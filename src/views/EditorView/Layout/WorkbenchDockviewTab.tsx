import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type ComponentType,
} from 'react';
import { useTranslation } from 'react-i18next';
import {
  VscClose,
  VscCloseAll,
  VscDatabase,
  VscError,
  VscGraphLine,
  VscInfo,
  VscInspect,
  VscLibrary,
  VscProject,
  VscOutput,
  VscPreview,
  VscSymbolEvent,
  VscSymbolMethod,
  VscTerminal,
} from 'react-icons/vsc';
import type { IconType } from 'react-icons';
import type { IDockviewPanelHeaderProps } from 'dockview-react';

import {
  requestCloseWorkbenchPanel,
  requestCloseWorkbenchPanels,
} from '@/features/application/editor/workbenchPanelClose';
import { buildTabContextMenuSections } from '@/features/application/editor/tabContextMenu';
import {
  isWorkbenchActivityViewId,
  isWorkbenchPersistentViewMetadata,
  layoutTabFromEditorMetadata,
  workbenchDockviewPort,
  type WorkbenchPanelInfo,
  type WorkbenchPanelMetadata,
  type WorkbenchPanelParams,
  type WorkbenchViewId,
} from '@/features/core/dockview';
import { resourceKey, useDocumentStateStore } from '@/features/core/resource';
import {
  ActionMenu,
  usePositionedActionMenu,
  type ActionMenuSection,
} from '@/shared/ui/actionMenu';

interface WorkbenchTabContextTarget {
  readonly panelInstanceId: string;
  readonly groupId: string;
  readonly metadata: WorkbenchPanelMetadata;
}

const VIEW_ICONS: Readonly<Record<WorkbenchViewId, IconType>> = {
  project: VscProject,
  nodes: VscLibrary,
  data: VscDatabase,
  commands: VscTerminal,
  details: VscInfo,
  inspect: VscInspect,
  logs: VscTerminal,
  output: VscOutput,
  diagnostics: VscError,
};

const VIEW_TITLE_KEYS = {
  project: 'activityBar.project',
  nodes: 'activityBar.nodes',
  data: 'activityBar.data',
  commands: 'activityBar.commands',
  details: 'panel.details',
  inspect: 'panel.inspect',
  logs: 'panel.logs',
  output: 'panel.output',
  diagnostics: 'panel.diagnostics',
} as const satisfies Record<WorkbenchViewId, string>;

function iconForMetadata(metadata: WorkbenchPanelMetadata): {
  readonly Icon: ComponentType<{ size?: number; 'aria-hidden'?: boolean }>;
  readonly key: string;
} {
  if (metadata.role === 'editor') {
    if (metadata.resourceKind === 'event') return { Icon: VscSymbolEvent, key: 'event' };
    if (metadata.resourceKind === 'function') return { Icon: VscSymbolMethod, key: 'function' };
    return { Icon: VscGraphLine, key: 'worksheet' };
  }
  if (metadata.role === 'result') return { Icon: VscPreview, key: 'result' };
  return { Icon: VIEW_ICONS[metadata.viewId], key: metadata.viewId };
}

function usePanelTitle(api: IDockviewPanelHeaderProps<WorkbenchPanelParams>['api']) {
  const [title, setTitle] = useState(api.title);

  useEffect(() => {
    const updateTitle = () => setTitle(api.title);
    const disposable = api.onDidTitleChange((event) => setTitle(event.title));
    updateTitle();
    return () => disposable.dispose();
  }, [api]);

  return title;
}

function useWorkbenchEdgeCollapsed(
  api: IDockviewPanelHeaderProps<WorkbenchPanelParams>['api'],
): boolean {
  const [groupApi, setGroupApi] = useState(() => api.group.api);
  const isEdge = groupApi.location.type === 'edge';
  const [collapsed, setCollapsed] = useState(
    () => isEdge && groupApi.isCollapsed(),
  );

  useEffect(() => {
    const updateGroup = () => setGroupApi(api.group.api);
    const groupDisposable = api.onDidGroupChange(updateGroup);
    updateGroup();
    return () => groupDisposable.dispose();
  }, [api]);

  useEffect(() => {
    setCollapsed(isEdge && groupApi.isCollapsed());
    if (!isEdge) return;

    const disposable = groupApi.onDidCollapsedChange(({ isCollapsed }) => {
      setCollapsed(isCollapsed);
    });
    return () => disposable.dispose();
  }, [groupApi, isEdge]);

  return isEdge && collapsed;
}

function titleForMetadata(
  metadata: WorkbenchPanelMetadata,
  panelTitle: string | undefined,
  translate: (key: string) => string,
): string {
  if (metadata.role === 'result') {
    return metadata.title || panelTitle || metadata.resultId;
  }
  if (metadata.role === 'editor') return panelTitle || metadata.resourceRef;
  return translate(VIEW_TITLE_KEYS[metadata.viewId]);
}

function genericContextMenuSections(
  target: WorkbenchTabContextTarget,
  closeLabel: string,
  closeGroupLabel: string,
): ActionMenuSection[] {
  return [{
    items: [
      {
        id: 'close',
        label: closeLabel,
        icon: <VscClose size={12} />,
        onClick: () => void requestCloseWorkbenchPanel(target.panelInstanceId),
      },
      {
        id: 'close-group',
        label: closeGroupLabel,
        icon: <VscCloseAll size={12} />,
        danger: true,
        onClick: () => {
          const panelInstanceIds = workbenchDockviewPort
            .listGroupPanels(target.groupId)
            .map((panel: WorkbenchPanelInfo) => panel.panelInstanceId);
          void requestCloseWorkbenchPanels(panelInstanceIds);
        },
      },
    ],
  }];
}

export function WorkbenchDockviewTab(
  props: IDockviewPanelHeaderProps<WorkbenchPanelParams>,
) {
  const { t } = useTranslation();
  const metadata = props.params.metadata;
  const panelTitle = usePanelTitle(props.api);
  const isEdgeCollapsed = useWorkbenchEdgeCollapsed(props.api);
  const title = titleForMetadata(metadata, panelTitle, t);
  const isActivityTab = metadata.role === 'view' && isWorkbenchActivityViewId(metadata.viewId);
  const isPersistentSidebarTab = isWorkbenchPersistentViewMetadata(metadata);
  const { Icon, key: iconKey } = iconForMetadata(metadata);
  const editorDocumentKey = metadata.role === 'editor'
    ? resourceKey({ id: metadata.resourceRef, kind: metadata.resourceKind })
    : null;
  const dirty = useDocumentStateStore((state) => (
    editorDocumentKey ? state.documents[editorDocumentKey]?.dirty === true : false
  ));
  const {
    contextMenu,
    setContextMenu,
    closeActionMenu,
  } = usePositionedActionMenu<WorkbenchTabContextTarget>();
  const tabContentRef = useRef<HTMLDivElement>(null);

  const requestClose = useCallback(() => {
    void requestCloseWorkbenchPanel(props.api.id);
  }, [props.api]);

  useEffect(() => {
    const tabContent = tabContentRef.current;
    if (!tabContent) return;

    if (isActivityTab) {
      const handleActivityClick = (event: MouseEvent) => {
        if (!props.api.group.api.isCollapsed()) return;
        event.preventDefault();
        event.stopPropagation();
        props.api.setActive();
        props.api.group.api.expand();
      };
      tabContent.addEventListener('click', handleActivityClick);
      return () => tabContent.removeEventListener('click', handleActivityClick);
    }

    if (isPersistentSidebarTab) return;

    const tabHost = tabContent.parentElement;
    if (!tabHost) return;

    let middleButtonDown = false;
    let middleCloseHandled = false;
    const handlePointerDown = (event: PointerEvent) => {
      if (event.button !== 1) return;
      middleButtonDown = true;
      middleCloseHandled = false;
      event.preventDefault();
      event.stopPropagation();
    };
    const handlePointerUp = (event: PointerEvent) => {
      if (!middleButtonDown || event.button !== 1) return;
      middleButtonDown = false;
      middleCloseHandled = true;
      event.preventDefault();
      event.stopPropagation();
      requestClose();
    };
    const handlePointerLeave = () => {
      middleButtonDown = false;
      middleCloseHandled = false;
    };
    const handleAuxClick = (event: MouseEvent) => {
      if (event.button !== 1) return;
      event.preventDefault();
      event.stopPropagation();
      if (!middleCloseHandled) requestClose();
      middleCloseHandled = false;
    };
    const handleContextMenu = (event: MouseEvent) => {
      event.preventDefault();
      event.stopPropagation();
      setContextMenu({
        x: event.clientX,
        y: event.clientY,
        target: {
          panelInstanceId: props.api.id,
          groupId: props.api.group.id,
          metadata,
        },
      });
    };

    tabHost.addEventListener('pointerdown', handlePointerDown);
    tabHost.addEventListener('pointerup', handlePointerUp);
    tabHost.addEventListener('pointerleave', handlePointerLeave);
    tabHost.addEventListener('auxclick', handleAuxClick);
    tabHost.addEventListener('contextmenu', handleContextMenu);
    return () => {
      tabHost.removeEventListener('pointerdown', handlePointerDown);
      tabHost.removeEventListener('pointerup', handlePointerUp);
      tabHost.removeEventListener('pointerleave', handlePointerLeave);
      tabHost.removeEventListener('auxclick', handleAuxClick);
      tabHost.removeEventListener('contextmenu', handleContextMenu);
    };
  }, [isActivityTab, isPersistentSidebarTab, metadata, props.api, requestClose, setContextMenu]);

  if (isActivityTab) {
    return (
      <div
        ref={tabContentRef}
        className="dv-default-tab"
        data-workbench-activity-tab
        data-workbench-activity-separator={metadata.viewId === 'commands' ? 'true' : undefined}
        data-workbench-tab-edge-collapsed={isEdgeCollapsed ? 'true' : undefined}
        data-panel-instance-id={props.api.id}
        aria-label={title}
        title={title}
      >
        <span data-workbench-activity-icon aria-hidden="true">
          <Icon size={18} />
        </span>
        <span className="sr-only">{title}</span>
      </div>
    );
  }

  const contextMenuSections = contextMenu
    ? contextMenu.target.metadata.role === 'editor'
      ? buildTabContextMenuSections(
          {
            panelInstanceId: contextMenu.target.panelInstanceId,
            groupId: contextMenu.target.groupId,
            tab: layoutTabFromEditorMetadata(contextMenu.target.metadata),
          },
          t,
        )
      : genericContextMenuSections(
          contextMenu.target,
          t('tabBar.contextMenu.close'),
          t('tabBar.closeGroup'),
        )
    : [];

  return (
    <>
      <div
        ref={tabContentRef}
        className="dv-default-tab"
        data-panel-instance-id={props.api.id}
        data-workbench-tab
        data-workbench-tab-edge-collapsed={isEdgeCollapsed ? 'true' : undefined}
        title={title}
      >
        <span className="dv-default-tab-content flex min-w-0 items-center gap-1.5">
          <span
            className="flex shrink-0 items-center text-muted-foreground"
            data-workbench-tab-icon={iconKey}
          >
            <Icon size={14} aria-hidden />
          </span>
          <span className="min-w-0 truncate" data-workbench-tab-title>{title}</span>
          {dirty ? (
            <span
              aria-hidden
              className="size-1.5 shrink-0 rounded-full bg-(--accent-color)"
              data-workbench-tab-dirty
            />
          ) : null}
        </span>
        {isPersistentSidebarTab ? null : (
          <button
            type="button"
            className="dv-default-tab-action"
            aria-label={t('tabBar.contextMenu.close')}
            data-workbench-tab-close
            onPointerDown={(event) => {
              event.preventDefault();
              event.stopPropagation();
            }}
            onClick={(event) => {
              event.preventDefault();
              event.stopPropagation();
              requestClose();
            }}
          >
            <VscClose aria-hidden />
          </button>
        )}
      </div>
      {contextMenu ? (
        <ActionMenu
          position={{ x: contextMenu.x, y: contextMenu.y }}
          sections={contextMenuSections}
          onClose={closeActionMenu}
        />
      ) : null}
    </>
  );
}
