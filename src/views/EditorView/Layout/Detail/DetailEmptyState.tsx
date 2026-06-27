import { useTranslation } from 'react-i18next';
import { DetailText } from './shared/DetailText';

export function DetailEmptyState() {
  const { t } = useTranslation();

  return (
    <div className="flex flex-1 flex-col items-center justify-center p-4 text-center opacity-30 group">
      <svg
        className="mb-2 h-12 w-12 text-muted-foreground transition-transform group-hover:scale-110"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth="1"
      >
        <path d="M13 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V9z" />
        <polyline points="13 2 13 9 20 9" />
      </svg>
      <DetailText tone="muted" className="font-bold uppercase tracking-widest">
        {t('detail.noSelection')}
      </DetailText>
      <DetailText tone="smallMuted" className="mt-1 italic">
        {t('detail.noSelectionHint')}
      </DetailText>
    </div>
  );
}
