import { create } from 'zustand';

export interface ClipboardPinEntry {
  pinId: string;
  name: string;
  direction: 'input' | 'output';
  userValue?: unknown;
}

export interface ClipboardEntry {
  nodeType: string;
  position: { x: number; y: number };
  params?: {
    variableId?: string;
    variableName?: string;
    variableType?: string;
    subGraphId?: string;
    dataframeId?: string;
  };
  pins: ClipboardPinEntry[];
}

export interface ClipboardSnapshot {
  entries: ClipboardEntry[];
  internalConnections: Array<{ fromPin: string; toPin: string }>;
}

interface ClipboardStore {
  clipboard: ClipboardSnapshot | null;
  setClipboard: (snapshot: ClipboardSnapshot) => void;
  clearClipboard: () => void;
}

export const useClipboardStore = create<ClipboardStore>((set) => ({
  clipboard: null,
  setClipboard: (snapshot) => set({ clipboard: snapshot }),
  clearClipboard: () => set({ clipboard: null }),
}));
