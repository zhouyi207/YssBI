import { useTranslation } from 'react-i18next';
import type { EditorSplitDirection } from '@/features/core/layout/editorSplitHitTest';
import { useEditorDropPreviewStore } from '@/features/application/editor/editorDropPreviewStore';
import {
  editorDropPreviewLabelClass,
  editorDropPreviewShellClass,
} from './editorDropPreviewStyles';

const SPLIT_EDGE_LABEL_KEYS: Record<EditorSplitDirection, string> = {
  left: 'editorDropPreview.splitLeft',
  right: 'editorDropPreview.splitRight',
  top: 'editorDropPreview.splitTop',
  bottom: 'editorDropPreview.splitBottom',
};

/** VS Code–style editor drop overlay — pointer-driven split halves and center merge. */
export function EditorDropPreviewOverlay() {
  const { t } = useTranslation();
  const preview = useEditorDropPreviewStore((state) => state.preview);
  if (!preview) return null;

  let label: string;
  if (preview.kind === 'function-into-event') {
    label = preview.shiftHeld
      ? t('editorDropPreview.dropFunctionIntoEventReady')
      : t('editorDropPreview.dropFunctionIntoEventHint');
  } else if (preview.kind === 'merge') {
    label = preview.resourceName
      ? t('editorDropPreview.openResource', { name: preview.resourceName })
      : t('editorDropPreview.mergeIntoGroup');
  } else {
    label = t(SPLIT_EDGE_LABEL_KEYS[preview.edge]);
  }

  return (
    <div
      className={`${editorDropPreviewShellClass} flex items-center justify-center`}
      style={{
        top: preview.rect.top,
        left: preview.rect.left,
        width: preview.rect.width,
        height: preview.rect.height,
      }}
    >
      <span className={editorDropPreviewLabelClass}>{label}</span>
    </div>
  );
}
