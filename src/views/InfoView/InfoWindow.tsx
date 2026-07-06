import type { FC } from 'react';
import { useTranslation } from 'react-i18next';
import { usePresentationWindow } from '@/features/application/presentation';
import { PresentationWindowShell } from '@/features/application/window/PresentationWindowShell';
import { ReportSourceView } from '@/features/core/resultSource/components/ReportSourceView';

const INFO_ICON = (
  <svg className="h-4 w-4 text-[var(--accent-color)]" fill="none" stroke="currentColor" viewBox="0 0 24 24">
    <path
      strokeLinecap="round"
      strokeLinejoin="round"
      strokeWidth={2}
      d="M9 17v-2m3 2v-4m3 4v-6m2 10H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z"
    />
  </svg>
);

export const InfoWindow: FC = () => {
  const { t } = useTranslation();
  const { state, isMaximized } = usePresentationWindow('info', 'InfoWindow');

  const title =
    state.status === 'ready'
      ? state.descriptor.title
      : t('info.regressionResults');

  return (
    <PresentationWindowShell
      logTag="InfoWindow"
      title={title}
      icon={INFO_ICON}
      state={state}
      isMaximized={isMaximized}
      errorMessages={{
        missingSourceId: t('info.missingDataKey'),
        notFound: t('info.noData'),
        loadFailed: t('info.failedInitialize'),
      }}
      contentClassName="flex min-h-0 flex-1 flex-col overflow-hidden"
    >
      {state.status === 'ready' && state.payload.mode === 'report' ? (
        <ReportSourceView
          payload={state.descriptor}
          layout="window"
          data={state.payload.data}
        />
      ) : null}
    </PresentationWindowShell>
  );
};
