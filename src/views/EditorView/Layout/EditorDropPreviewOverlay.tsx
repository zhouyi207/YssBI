import { useTranslation } from 'react-i18next';
import type { EditorSplitEdge } from '@/features/core/layout/editorSplitLayout';
import { useEditorDropPreviewStore } from '@/features/application/editor/editorDropPreviewStore';
import {
  editorDropPreviewLabelClass,
  editorDropPreviewShellClass,
} from './editorDropPreviewStyles';

const SPLIT_EDGE_LABEL_KEYS: Record<EditorSplitEdge, string> = {
  left: 'editorDropPreview.splitLeft',
  right: 'editorDropPreview.splitRight',
  top: 'editorDropPreview.splitTop',
  bottom: 'editorDropPreview.splitBottom',
  center: 'editorDropPreview.splitRight',
};

/** Unified editor drop preview — tab split halves + sidebar graph open on canvas/watermark. */
export function EditorDropPreviewOverlay() {
  const { t } = useTranslation();
  const preview = useEditorDropPreviewStore((state) => state.preview);
  if (!preview) return null;

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
      {preview.kind === 'canvas-open' ? (
        <span className={editorDropPreviewLabelClass}>
          {t('editorDropPreview.openResource', { name: preview.resourceName })}
        </span>
      ) : (
        <span className={editorDropPreviewLabelClass}>
          {t(SPLIT_EDGE_LABEL_KEYS[preview.edge])}
        </span>
      )}
    </div>
  );
}
