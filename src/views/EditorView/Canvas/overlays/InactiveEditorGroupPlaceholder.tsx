import { useTranslation } from 'react-i18next';

/** Lightweight shell for split editor groups that are visible but not active. */
export function InactiveEditorGroupPlaceholder() {
  const { t } = useTranslation();
  return (
    <div className="flex h-full w-full items-center justify-center bg-[var(--workbench-bg)] text-sm text-muted-foreground select-none">
      {t('editor.inactiveGroupPlaceholder')}
    </div>
  );
}
