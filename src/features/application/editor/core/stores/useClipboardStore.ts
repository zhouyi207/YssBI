import { create } from 'zustand';
import { Node } from '@/shared/types/ui';

interface ClipboardStore {
  clipboard: Node[];
  setClipboard: (nodes: Node[]) => void;
  clearClipboard: () => void;
}

export const useClipboardStore = create<ClipboardStore>((set) => ({
  clipboard: [],
  setClipboard: (nodes) => set({ clipboard: nodes }),
  clearClipboard: () => set({ clipboard: [] }),
}));
