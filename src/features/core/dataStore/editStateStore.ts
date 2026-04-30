import { create } from 'zustand';
import { DatabaseId } from '@/shared/types/domain/ids';
import type { EditState } from '@/shared/types/domain/dataframe';
export type { EditState } from '@/shared/types/domain/dataframe';

interface EditStateStore {
  editStateByDatabase: Record<DatabaseId, EditState>;

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
  editStateByDatabase: {},

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

  clear: () => set({ editStateByDatabase: {} }),
}));

export { EMPTY_STATE as EMPTY_EDIT_STATE };
