import { create } from "zustand";

export interface HistoryStoreState {
  canUndo: boolean;
  canRedo: boolean;
  pending: boolean;
}

export const EMPTY_HISTORY_STATE: HistoryStoreState = {
  canUndo: false,
  canRedo: false,
  pending: false,
};

export const useHistoryStore = create<HistoryStoreState>(() => EMPTY_HISTORY_STATE);
