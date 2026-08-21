import { describe, expect, it } from 'vitest';
import { getGridSelectionPrimaryCellText } from './gridSelectionCellPreview';
import {
  createSelectAllSelection,
  selectedRowIndicesFromSelection,
  type DatabaseGridSelection,
} from './useSelection';

describe('database grid selection', () => {
  it('projects only destructive row selections and creates select-all selection', () => {
    const explicitRows: DatabaseGridSelection = {
      type: 'rows',
      rows: [3, -1, 1, 3, 5],
    };
    const cellRanges: DatabaseGridSelection = {
      type: 'cells',
      activeCell: { row: 1, column: 1 },
      ranges: [
        { row: 1, column: 1, rowCount: 2, columnCount: 2 },
        { row: -1, column: 0, rowCount: 4, columnCount: 3 },
        { row: 2, column: 0, rowCount: 4, columnCount: 4 },
      ],
    };

    expect(selectedRowIndicesFromSelection(explicitRows, 3, 5)).toEqual([3, 1]);
    expect(selectedRowIndicesFromSelection(cellRanges, 3, 5)).toEqual([0, 1, 2, 3, 4]);

    const selectAll = createSelectAllSelection(3, 5);
    expect(selectAll).toEqual({
      type: 'cells',
      activeCell: { row: 0, column: 0 },
      ranges: [{ row: 0, column: 0, rowCount: 5, columnCount: 3 }],
    });
    expect(selectedRowIndicesFromSelection(selectAll, 3, 5)).toEqual([0, 1, 2, 3, 4]);
  });

  it('previews the primary cell for cell, row, and column selections', () => {
    const loadedRows = [
      ['r0c0', 'r0c1'],
      ['r1c0', 'r1c1'],
    ];

    expect(getGridSelectionPrimaryCellText({
      type: 'cells',
      activeCell: { row: 1, column: 1 },
      ranges: [{ row: 0, column: 0, rowCount: 2, columnCount: 2 }],
    }, 2, 2, loadedRows)).toBe('r1c1');
    expect(getGridSelectionPrimaryCellText({ type: 'rows', rows: [1] }, 2, 2, loadedRows))
      .toBe('r1c0');
    expect(getGridSelectionPrimaryCellText({ type: 'columns', columns: [1] }, 2, 2, loadedRows))
      .toBe('r0c1');
  });
});
