import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { useHistoryStore } from '@/features/core/history';
import type { HistoryEntry } from '@/features/core/history';
import { useActiveEditorGroup } from '@/features/core/editor/hooks/useActiveEditorGroup';
import {
  buildCommandsFlatRows,
  useSidebarSectionExpandSnapshot,
  useSidebarStore,
} from '@/features/core/sidebar';
import { SidebarTabPanel } from '../sections/SidebarTabPanel';
import { SidebarFlatRowPanel } from '../sections/SidebarFlatRowPanel';
import { noopSidebarHandler } from '../sections/sidebarFlatRowContext';

const EMPTY_STACK: HistoryEntry[] = [];

export function SidebarCommandsTab() {
  const { t } = useTranslation();
  const sectionExpanded = useSidebarSectionExpandSnapshot('commandsUndo', 'commandsRedo');
  const toggleSection = useSidebarStore((s) => s.toggleSection);

  const { activeTabId } = useActiveEditorGroup();

  const undoStack = useHistoryStore((s) =>
    activeTabId ? s.histories[activeTabId]?.undoStack ?? EMPTY_STACK : EMPTY_STACK,
  );
  const redoStack = useHistoryStore((s) =>
    activeTabId ? s.histories[activeTabId]?.redoStack ?? EMPTY_STACK : EMPTY_STACK,
  );

  const rows = useMemo(
    () =>
      buildCommandsFlatRows({
        undoStack,
        redoStack,
        hasActiveTab: Boolean(activeTabId),
        expandedSections: sectionExpanded,
        labels: {
          undo: `${t('common.undo')} (${undoStack.length})`,
          redo: `${t('common.redo')} (${redoStack.length})`,
          noHistory: t('sidebar.noCommandHistory'),
          noActiveGraph: t('sidebar.noActiveGraph'),
        },
      }),
    [activeTabId, redoStack, sectionExpanded, t, undoStack],
  );

  return (
    <SidebarTabPanel>
      <SidebarFlatRowPanel
        rows={rows}
        onToggleSection={toggleSection}
        onToggleGroup={noopSidebarHandler}
      />
    </SidebarTabPanel>
  );
}
