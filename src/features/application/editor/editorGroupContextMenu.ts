import type { TFunction } from 'i18next';
import type { ContextMenuSection } from '@/shared/ui/contextMenu';
import { closeEditorGroup, splitEditorGroup } from './tabCommands';
import type { EditorGroupToolbarActionId } from './editorGroupToolbarActions';

export function buildEditorGroupOverflowMenuSections(
  groupId: string,
  t: TFunction,
  options?: {
    includeActions?: ReadonlySet<EditorGroupToolbarActionId>;
  },
): ContextMenuSection[] {
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
  if (closeItems.length > 0) sections.push({ items: closeItems });

  return sections;
}
