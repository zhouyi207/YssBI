import { useTranslation } from 'react-i18next';

/** Placeholder for future Output channel (VS Code panel parity). */
export function OutputPanel() {
  const { t } = useTranslation();
  return (
    <div className="flex h-full w-full items-center justify-center text-sm text-muted-foreground select-none">
      {t('panel.outputPlaceholder')}
    </div>
  );
}
