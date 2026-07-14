import React, { Fragment, useCallback, useMemo } from 'react';
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
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip';
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

  const showOverflow = prepared.secondary.length > 0;

  return (
    <div className={editorTabBarActionsClass} data-editor-group-actions={groupId}>
      {prepared.primary.map((actionId) => renderPrimaryAction(actionId))}

      {showOverflow ? (
        <DropdownMenu>
          <Tooltip>
            <DropdownMenuTrigger asChild>
              <TooltipTrigger asChild>
                <Button
                  type="button"
                  variant="ghost"
                  size="icon-sm"
                  className="text-muted-foreground"
                  onPointerDown={stopEditorActionPointerDown}
                  aria-label={t('tabBar.overflow.title')}
                >
                  <VscEllipsis size={16} />
                </Button>
              </TooltipTrigger>
            </DropdownMenuTrigger>
            <TooltipContent side="bottom">{t('tabBar.overflow.title')}</TooltipContent>
          </Tooltip>
          <DropdownMenuContent align="end" className="min-w-[13.5rem]">
            {overflowSections.map((section, sectionIndex) => (
              <Fragment key={section.items.map((item) => item.id).join('-') || `section-${sectionIndex}`}>
                {sectionIndex > 0 ? <DropdownMenuSeparator /> : null}
                {section.items.map((item) => (
                  <DropdownMenuItem
                    key={item.id}
                    disabled={item.disabled}
                    title={item.disabled ? item.title : undefined}
                    variant={item.danger ? 'destructive' : 'default'}
                    onSelect={() => item.onClick?.()}
                    className="text-[12px]"
                  >
                    {item.icon}
                    {item.label}
                  </DropdownMenuItem>
                ))}
              </Fragment>
            ))}
          </DropdownMenuContent>
        </DropdownMenu>
      ) : null}
    </div>
  );
};
