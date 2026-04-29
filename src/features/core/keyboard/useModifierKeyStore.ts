import { create } from "zustand";

interface ModifierKeyState {
  altKey: boolean;
  ctrlKey: boolean;
  setModifierKeys: (keys: { altKey: boolean; ctrlKey: boolean }) => void;
  resetModifierKeys: () => void;
}

export const useModifierKeyStore = create<ModifierKeyState>((set) => ({
  altKey: false,
  ctrlKey: false,
  setModifierKeys: ({ altKey, ctrlKey }) => set({ altKey, ctrlKey }),
  resetModifierKeys: () => set({ altKey: false, ctrlKey: false }),
}));
