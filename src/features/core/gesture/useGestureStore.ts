import { create } from 'zustand';

interface GestureState {
  suppressNextContextMenu: boolean;
  clearGesture: (hadMovement?: boolean) => void;
  consumeSuppressContextMenu: () => boolean;
}

export const useGestureStore = create<GestureState>((set, get) => ({
  suppressNextContextMenu: false,
  clearGesture: (hadMovement = false) => set({ suppressNextContextMenu: hadMovement }),
  consumeSuppressContextMenu: () => {
    const value = get().suppressNextContextMenu;
    if (value) set({ suppressNextContextMenu: false });
    return value;
  },
}));
