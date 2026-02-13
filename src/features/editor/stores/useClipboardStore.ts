import { create } from 'zustand';
import { BaseNode } from '@/shared/types/editor';

interface ClipboardStore {
  clipboard: BaseNode[];
  setClipboard: (nodes: BaseNode[]) => void;
  clearClipboard: () => void;
}

export const useClipboardStore = create<ClipboardStore>((set) => ({
  clipboard: [],
  setClipboard: (nodes) => set({ clipboard: nodes }),
  clearClipboard: () => set({ clipboard: [] }),
}));
