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
    canDemoteVariable: true,
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
  it('disables demotion with a reason when no graph scope is active', () => {
    const actions = { ...sidebarActions(), canDemoteVariable: false };
    const sections = buildSidebarContextMenuSections({
      x: 10,
      y: 20,
      target: { type: 'variable', id: 'variable-1', name: 'Counter', isGlobal: true },
    }, actions, t);

    expect(sections.flatMap((section) => section.items).find((item) => item.id === 'demote-to-local'))
      .toMatchObject({ disabled: true, title: 'sidebar.noActiveGraph' });
  });

  it('offers both variable scopes from the Variables folder', () => {
    const actions = sidebarActions();
    const sections = buildSidebarContextMenuSections({
      x: 10,
      y: 20,
      target: { type: 'variableSection' },
    }, actions, t);
    const items = sections.flatMap((section) => section.items);

    expect(items.map((item) => item.id)).toEqual([
      'new-local-variable',
      'new-global-variable',
    ]);

    items.find((item) => item.id === 'new-local-variable')?.onClick?.();
    items.find((item) => item.id === 'new-global-variable')?.onClick?.();
    expect(actions.addVariable).toHaveBeenNthCalledWith(1, 'New Variable', 'Int64', false);
    expect(actions.addVariable).toHaveBeenNthCalledWith(2, 'New Variable', 'Int64', true);
  });

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
