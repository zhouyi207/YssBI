import { create } from 'zustand';
import { DatabaseId } from '@/shared/types/domain/ids';
import type { EditingCell, EditState } from '@/shared/types/domain/dataframe';
export type { EditingCell, EditState } from '@/shared/types/domain/dataframe';

interface EditStateStore {
  editingCell: EditingCell | null;
  editStateByDatabase: Record<DatabaseId, EditState>;

  setEditingCell(cell: EditingCell | null): void;
  clearEditingCell(): void;
  updateEditState(dbId: DatabaseId, state: EditState): void;
  clearEditState(dbId: DatabaseId): void;
  clear(): void;
}

const EMPTY_STATE: EditState = {
  canUndo: false,
  canRedo: false,
  isModified: false,
  undoCount: 0,
  redoCount: 0,
};

export const useEditStateStore = create<EditStateStore>((set) => ({
  editingCell: null,
  editStateByDatabase: {},

  setEditingCell: (cell) => set({ editingCell: cell }),
  clearEditingCell: () => set({ editingCell: null }),

  updateEditState: (dbId, state) =>
    set((s) => ({
      editStateByDatabase: {
        ...s.editStateByDatabase,
        [dbId]: state,
      },
    })),

  clearEditState: (dbId) =>
    set((s) => {
      const next = { ...s.editStateByDatabase };
      delete next[dbId];
      return { editStateByDatabase: next };
    }),

  clear: () => set({ editingCell: null, editStateByDatabase: {} }),
}));

export { EMPTY_STATE as EMPTY_EDIT_STATE };
