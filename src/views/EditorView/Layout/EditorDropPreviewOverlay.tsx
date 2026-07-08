import { useTranslation } from 'react-i18next';
import { useEditorDropPreviewStore } from '@/features/application/editor/editorDropPreviewStore';
import {
  editorDropPreviewLabelClass,
  editorDropPreviewShellClass,
} from './editorDropPreviewStyles';

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
      ) : null}
    </div>
  );
}
