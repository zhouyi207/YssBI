import { useTranslation } from 'react-i18next';

export function WorksheetEmptyState({ messageKey }: { messageKey?: string }) {
  const { t } = useTranslation();
  return (
    <div className="flex h-full w-full min-h-0 flex-col items-center justify-center p-8 text-center opacity-60">
      <svg
        className="mb-3 h-14 w-14 text-muted-foreground"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth="1"
      >
        <path d="M3 3v18h18" />
        <path d="M7 14l4-4 3 3 5-6" />
      </svg>
      <p className="text-sm font-medium text-foreground">{t(messageKey ?? 'worksheet.previewEmpty')}</p>
      <p className="mt-1 text-xs text-muted-foreground">{t('worksheet.previewEmptyHint')}</p>
    </div>
  );
}
