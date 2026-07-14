import React, { useCallback, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  VscSplitHorizontal,
  VscSplitVertical,
  VscChromeClose,
  VscEllipsis,
  VscLock,
  VscUnlock,
} from 'react-icons/vsc';
import { Button } from '@/components/ui/button';
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip';
import { ContextMenu } from '@/shared/ui/contextMenu';
import type { ContextMenuPosition } from '@/shared/ui/contextMenu';
import { editorTabBarActionsClass } from './editorTabStyles';
import {
  closeEditorGroup,
  splitEditorGroupFromPointer,
  toggleEditorGroupLocked,
} from '@/features/application/editor/tabCommands';
import {
  prepareEditorGroupToolbarActions,
  type EditorGroupToolbarActionId,
} from '@/features/application/editor/editorGroupToolbarActions';
import { buildEditorGroupOverflowMenuSections } from '@/features/application/editor/editorGroupContextMenu';
import { useModifierKeyStore } from '@/features/core/keyboard';
import { useSettingsStore } from '@/features/core/settings/settingsStore';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { useEditorTabStore } from '@/features/core/layout/editorTabStore';

interface EditorGroupToolbarProps {
  groupId: string;
}

function stopEditorActionPointerDown(e: React.PointerEvent): void {
  if (e.button !== 0) return;
  e.stopPropagation();
}

export const EditorGroupToolbar: React.FC<EditorGroupToolbarProps> = ({ groupId }) => {
  const { t } = useTranslation();
  const altKey = useModifierKeyStore((s) => s.altKey);
  const alwaysShowEditorActions = useSettingsStore((s) => s.editor.alwaysShowEditorActions ?? false);
  const isGroupActive = useLayoutStore((s) => s.activeEditorGroupId === groupId);
  const locked = useEditorTabStore((s) => s.getPlacement(groupId).locked === true);
  const [overflowMenu, setOverflowMenu] = useState<ContextMenuPosition | null>(null);

  const prepared = useMemo(
    () => prepareEditorGroupToolbarActions({
      isGroupActive,
      alwaysShowEditorActions,
      locked,
    }),
    [isGroupActive, alwaysShowEditorActions, locked],
  );

  const handleSplitPointer = useCallback((e: React.MouseEvent) => {
    e.stopPropagation();
    void splitEditorGroupFromPointer(groupId, e.altKey || altKey);
  }, [groupId, altKey]);

  const handleCloseGroup = useCallback((e: React.MouseEvent) => {
    e.stopPropagation();
    void closeEditorGroup(groupId);
  }, [groupId]);

  const handleToggleLock = useCallback((e: React.MouseEvent) => {
    e.stopPropagation();
    toggleEditorGroupLocked(groupId);
  }, [groupId]);

  const renderPrimaryAction = (actionId: EditorGroupToolbarActionId) => {
    switch (actionId) {
      case 'split-pointer':
        return (
          <Tooltip key={actionId}>
            <TooltipTrigger asChild>
              <Button
                type="button"
                variant="ghost"
                size="icon-sm"
                className="text-muted-foreground"
                onPointerDown={stopEditorActionPointerDown}
                onClick={handleSplitPointer}
              >
                {altKey ? <VscSplitVertical size={15} /> : <VscSplitHorizontal size={15} />}
              </Button>
            </TooltipTrigger>
            <TooltipContent side="bottom">
              {altKey ? t('tabBar.splitDownAlt') : t('tabBar.splitRight')}
            </TooltipContent>
          </Tooltip>
        );
      case 'close-group':
        return (
          <Tooltip key={actionId}>
            <TooltipTrigger asChild>
              <Button
                type="button"
                variant="ghost"
                size="icon-sm"
                className="text-muted-foreground hover:text-red-400"
                onPointerDown={stopEditorActionPointerDown}
                onClick={handleCloseGroup}
              >
                <VscChromeClose size={15} />
              </Button>
            </TooltipTrigger>
            <TooltipContent side="bottom">{t('tabBar.closeGroup')}</TooltipContent>
          </Tooltip>
        );
      case 'toggle-lock':
        return (
          <Tooltip key={actionId}>
            <TooltipTrigger asChild>
              <Button
                type="button"
                variant="ghost"
                size="icon-sm"
                className="text-muted-foreground"
                onPointerDown={stopEditorActionPointerDown}
                onClick={handleToggleLock}
                aria-label={locked ? t('tabBar.overflow.unlockGroup') : t('tabBar.overflow.lockGroup')}
              >
                {locked ? <VscLock size={14} /> : <VscUnlock size={14} />}
              </Button>
            </TooltipTrigger>
            <TooltipContent side="bottom">
              {locked ? t('tabBar.overflow.unlockGroup') : t('tabBar.overflow.lockGroup')}
            </TooltipContent>
          </Tooltip>
        );
      default:
        return null;
    }
  };

  const overflowSections = useMemo(
    () => buildEditorGroupOverflowMenuSections(groupId, t, {
      locked,
      includeActions: new Set(prepared.secondary),
    }),
    [groupId, t, locked, prepared.secondary],
  );

  const openOverflowMenu = useCallback((e: React.MouseEvent<HTMLButtonElement>) => {
    e.stopPropagation();
    const rect = e.currentTarget.getBoundingClientRect();
    setOverflowMenu({
      x: rect.right,
      y: rect.bottom,
      placement: 'below-end',
    });
  }, []);

  const showOverflow = prepared.secondary.length > 0;

  return (
    <>
      <div className={editorTabBarActionsClass} data-editor-group-actions={groupId}>
        {prepared.primary.map((actionId) => renderPrimaryAction(actionId))}

        {showOverflow ? (
          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                type="button"
                variant="ghost"
                size="icon-sm"
                className="text-muted-foreground"
                onPointerDown={stopEditorActionPointerDown}
                onClick={openOverflowMenu}
                aria-label={t('tabBar.overflow.title')}
                aria-haspopup="menu"
                aria-expanded={overflowMenu != null}
              >
                <VscEllipsis size={16} />
              </Button>
            </TooltipTrigger>
            <TooltipContent side="bottom">{t('tabBar.overflow.title')}</TooltipContent>
          </Tooltip>
        ) : null}
      </div>

      {overflowMenu ? (
        <ContextMenu
          position={overflowMenu}
          sections={overflowSections}
          onClose={() => setOverflowMenu(null)}
        />
      ) : null}
    </>
  );
};
