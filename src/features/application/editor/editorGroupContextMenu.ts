import type { TFunction } from 'i18next';
import type { ContextMenuSection } from '@/shared/ui/contextMenu';
import { closeEditorGroup, splitEditorGroup, toggleEditorGroupLocked } from './tabCommands';
import type { EditorGroupToolbarActionId } from './editorGroupToolbarActions';

export function buildEditorGroupOverflowMenuSections(
  groupId: string,
  t: TFunction,
  options?: {
    locked?: boolean;
    includeActions?: ReadonlySet<EditorGroupToolbarActionId>;
  },
): ContextMenuSection[] {
  const locked = options?.locked === true;
  const include = options?.includeActions;
  const shouldInclude = (id: EditorGroupToolbarActionId) => !include || include.has(id);

  const splitItems = [];
  if (shouldInclude('split-right')) {
    splitItems.push({
      id: 'split-right',
      label: t('tabBar.overflow.splitRight'),
      onClick: () => void splitEditorGroup(groupId, 'row'),
    });
  }
  if (shouldInclude('split-down')) {
    splitItems.push({
      id: 'split-down',
      label: t('tabBar.overflow.splitDown'),
      onClick: () => void splitEditorGroup(groupId, 'col'),
    });
  }

  const lockItems = shouldInclude('toggle-lock')
    ? [{
        id: 'toggle-lock',
        label: locked ? t('tabBar.overflow.unlockGroup') : t('tabBar.overflow.lockGroup'),
        onClick: () => toggleEditorGroupLocked(groupId),
      }]
    : [];

  const closeItems = shouldInclude('close-group')
    ? [{
        id: 'close-group',
        label: t('tabBar.overflow.closeGroup'),
        danger: true,
        onClick: () => void closeEditorGroup(groupId),
      }]
    : [];

  const sections: ContextMenuSection[] = [];
  if (splitItems.length > 0) sections.push({ items: splitItems });
  if (lockItems.length > 0) sections.push({ items: lockItems });
  if (closeItems.length > 0) sections.push({ items: closeItems });

  return sections;
}
