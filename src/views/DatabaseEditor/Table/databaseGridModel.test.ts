import { describe, expect, it } from 'vitest';
import { createKeyboardCellSelection } from './databaseGridModel';

describe('database grid keyboard selection', () => {
  it('extends from the stable anchor only while Shift is held', () => {
    const initial = createKeyboardCellSelection(null, null, { row: 1, column: 1 }, false);
    expect(initial).toEqual({
      anchor: { row: 1, column: 1 },
      selection: {
        type: 'cells',
        activeCell: { row: 1, column: 1 },
        ranges: [{ row: 1, column: 1, rowCount: 1, columnCount: 1 }],
      },
    });

    const extended = createKeyboardCellSelection(
      initial.selection,
      initial.anchor,
      { row: 1, column: 3 },
      true,
    );
    expect(extended).toEqual({
      anchor: { row: 1, column: 1 },
      selection: {
        type: 'cells',
        activeCell: { row: 1, column: 3 },
        ranges: [{ row: 1, column: 1, rowCount: 1, columnCount: 3 }],
      },
    });

    expect(createKeyboardCellSelection(
      extended.selection,
      extended.anchor,
      { row: 2, column: 3 },
      false,
    ).anchor).toEqual({ row: 2, column: 3 });
  });
});
