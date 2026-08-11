import type { TFunction } from 'i18next';
import { describe, expect, it, vi } from 'vitest';
import { buildSidebarContextMenuSections } from './buildSidebarContextMenuSections';

const t = ((key: string) => key) as TFunction;

function sidebarActions() {
  return {
    openGraph: vi.fn(),
    createGraph: vi.fn(),
    renameGraphItem: vi.fn(),
    deleteGraphItem: vi.fn(),
    duplicateGraphItem: vi.fn(),
    addVariable: vi.fn(),
    renameVariableItem: vi.fn(),
    deleteVariable: vi.fn(),
    promoteVariable: vi.fn(),
    demoteVariable: vi.fn(),
    openDatabase: vi.fn(),
    renameDatabaseItem: vi.fn(),
    deleteDatabaseItem: vi.fn(),
    importData: vi.fn(),
    openWorksheet: vi.fn(),
    renameWorksheetItem: vi.fn(),
    duplicateWorksheet: vi.fn(),
    deleteWorksheet: vi.fn(),
    addWorksheet: vi.fn(),
    revealInExplorer: vi.fn(),
  };
}

describe('buildSidebarContextMenuSections', () => {
  it('exposes authoritative worksheet rename with the opaque path and Rust-provided name', () => {
    const actions = sidebarActions();
    const sections = buildSidebarContextMenuSections({
      x: 10,
      y: 20,
      target: {
        type: 'worksheet',
        worksheetPath: 'worksheets/Report.yssbi-worksheet',
        name: 'Report',
      },
    }, actions, t);
    const items = sections.flatMap((section) => section.items);

    expect(items.map((item) => item.id)).toEqual([
      'open',
      'reveal-in-explorer',
      'rename',
      'duplicate',
      'delete',
    ]);

    items.find((item) => item.id === 'rename')?.onClick?.();
    expect(actions.renameWorksheetItem).toHaveBeenCalledWith(
      'worksheets/Report.yssbi-worksheet',
      'Report',
    );
  });
});
