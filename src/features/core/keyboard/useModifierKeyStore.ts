import { create } from "zustand";

interface ModifierKeyState {
  altKey: boolean;
  ctrlKey: boolean;
  shiftKey: boolean;
  setModifierKeys: (keys: { altKey: boolean; ctrlKey: boolean; shiftKey: boolean }) => void;
  resetModifierKeys: () => void;
}

export const useModifierKeyStore = create<ModifierKeyState>((set) => ({
  altKey: false,
  ctrlKey: false,
  shiftKey: false,
  setModifierKeys: ({ altKey, ctrlKey, shiftKey }) => set({ altKey, ctrlKey, shiftKey }),
  resetModifierKeys: () => set({ altKey: false, ctrlKey: false, shiftKey: false }),
}));
