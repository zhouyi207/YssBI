import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  type ComponentProps,
  type ComponentType,
  type RefAttributes,
} from 'react';
import { useTranslation } from 'react-i18next';
import {
  DockviewDefaultTab,
  type IDockviewPanelHeaderProps,
} from 'dockview-react';

import { closeEditorTab } from '@/features/application/editor/closeEditorTab';
import { buildTabContextMenuSections } from '@/features/application/editor/tabContextMenu';
import type { DockviewPanelParams } from '@/features/core/dockview';
import { ActionMenu, usePositionedActionMenu } from '@/shared/ui/actionMenu';
import type { LayoutTab } from '@/shared/types';

interface EditorTabContextTarget {
  groupId: string;
  tab: LayoutTab;
}

const DockviewDefaultTabWithRef = DockviewDefaultTab as ComponentType<
  ComponentProps<typeof DockviewDefaultTab> & RefAttributes<HTMLDivElement>
>;

function readLayoutTab(params: DockviewPanelParams): LayoutTab | null {
  const value = params.layoutTab.data?.layoutTab;
  return value && typeof value === 'object' ? value as LayoutTab : null;
}

export function DockviewEditorTab(
  props: IDockviewPanelHeaderProps<DockviewPanelParams>,
) {
  const { t } = useTranslation();
  const {
    contextMenu,
    setContextMenu,
    closeActionMenu,
  } = usePositionedActionMenu<EditorTabContextTarget>();
  const tabContentRef = useRef<HTMLDivElement>(null);
  const requestClose = useCallback(() => {
    const tab = props.params.layoutTab;
    void closeEditorTab(tab.resourceRef, props.api.group.id);
  }, [props.api, props.params.layoutTab]);

  const actionMenuSections = useMemo(
    () => contextMenu
      ? buildTabContextMenuSections(
          contextMenu.target.groupId,
          contextMenu.target.tab,
          t,
        )
      : [],
    [contextMenu, t],
  );

  useEffect(() => {
    const tabShell = tabContentRef.current?.closest<HTMLElement>('.dv-tab');
    if (!tabShell) return;

    const handleContextMenu = (event: MouseEvent) => {
      const tab = readLayoutTab(props.params);
      if (!tab) return;

      event.preventDefault();
      event.stopPropagation();
      setContextMenu({
        x: event.clientX,
        y: event.clientY,
        target: { groupId: props.api.group.id, tab },
      });
    };

    tabShell.addEventListener('contextmenu', handleContextMenu);
    return () => tabShell.removeEventListener('contextmenu', handleContextMenu);
  }, [props.api.group.id, props.params, setContextMenu]);

  return (
    <>
      <DockviewDefaultTabWithRef
        {...props}
        ref={tabContentRef}
        closeActionOverride={requestClose}
      />
      {contextMenu ? (
        <ActionMenu
          position={{ x: contextMenu.x, y: contextMenu.y }}
          sections={actionMenuSections}
          onClose={closeActionMenu}
        />
      ) : null}
    </>
  );
}
