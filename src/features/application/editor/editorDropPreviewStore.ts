import { create } from 'zustand';
import type { EditorSplitDirection } from '@/features/core/layout/editorSplitHitTest';
import type { EditorDropPreviewRect } from '@/features/core/layout/editorDropPreview';

export type EditorDropPreview =
  | {
      kind: 'split';
      targetGroupId: string;
      edge: EditorSplitDirection;
      rect: EditorDropPreviewRect;
    }
  | {
      kind: 'merge';
      targetGroupId: string;
      rect: EditorDropPreviewRect;
      resourceName?: string;
    }
  | {
      kind: 'function-into-event';
      targetGroupId: string;
      rect: EditorDropPreviewRect;
      shiftHeld: boolean;
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
