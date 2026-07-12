import { create } from 'zustand';
import type { EditorSplitEdge } from '@/features/core/layout/editorSplitLayout';
import type { EditorDropPreviewRect } from '@/features/core/layout/editorDropPreview';

export type EditorDropPreview =
  | {
      kind: 'split';
      targetGroupId: string;
      edge: EditorSplitEdge;
      rect: EditorDropPreviewRect;
    }
  | {
      kind: 'canvas-open';
      targetGroupId: string;
      rect: EditorDropPreviewRect;
      resourceName: string;
    };

interface EditorDropPreviewStore {
  preview: EditorDropPreview | null;
  setPreview: (preview: EditorDropPreview) => void;
  clearPreview: () => void;
}

export const useEditorDropPreviewStore = create<EditorDropPreviewStore>((set) => ({
  preview: null,
  setPreview: (preview) => set({ preview }),
  clearPreview: () => set({ preview: null }),
}));
