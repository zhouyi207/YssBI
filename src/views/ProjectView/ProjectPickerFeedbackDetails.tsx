import { useTranslation } from 'react-i18next';
import type {
  ProjectPickerErrorPresentation,
  ProjectPickerRecoveryPresentation,
} from '@/features/application/project';

export function ProjectPickerErrorDetails({
  error,
}: {
  error: ProjectPickerErrorPresentation;
}) {
  const { t } = useTranslation();
  const message = t(error.messageKey, {
    defaultValue: t(error.fallbackMessageKey),
  });

  return (
    <span className="flex min-w-0 flex-col gap-1">
      <span>{message}</span>
      <span className="break-all font-mono text-[11px] text-muted-foreground">
        {error.code}
        {error.incidentId ? ` · ${error.incidentId}` : null}
      </span>
    </span>
  );
}

export function ProjectPickerRecoveryDetails({
  recovery,
}: {
  recovery: ProjectPickerRecoveryPresentation;
}) {
  const { t } = useTranslation();

  return (
    <span className="flex min-w-0 flex-col gap-1">
      <span>{t(recovery.messageKey, { defaultValue: recovery.action })}</span>
      {recovery.path ? (
        <span className="break-all font-mono text-[11px] text-muted-foreground">
          {recovery.path}
        </span>
      ) : null}
    </span>
  );
}

export function ProjectPickerStaleDetails() {
  const { t } = useTranslation();
  return (
    <span>
      {t('projectPicker.issues.stale', { defaultValue: t('common.error') })}
    </span>
  );
}
