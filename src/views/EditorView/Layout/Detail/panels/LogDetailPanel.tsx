import { useTranslation } from 'react-i18next';
import { LogLevel, LogType } from '@/shared/types/ui';
import { DetailPanelShell } from '../shared/DetailPanelShell';
import { DetailForm, DetailReadonlyField } from '../shared/DetailForm';
import { DetailCollapsibleSection } from '../shared/DetailCollapsibleSection';
import { DetailFieldRow } from '../shared/DetailFieldRow';
import { DetailBadge } from '../shared/DetailText';
import { DetailText } from '../shared/DetailText';

const getLevelColor = (level: LogLevel) => {
  switch (level) {
    case 'error':
      return 'text-red-400';
    case 'warn':
      return 'text-yellow-400';
    case 'info':
      return 'text-blue-400';
    case 'debug':
      return 'text-muted-foreground';
    case 'trace':
      return 'text-muted-foreground/70';
    default:
      return 'text-muted-foreground';
  }
};

const getTypeColor = (type: LogType) => {
  switch (type) {
    case 'application':
      return 'text-green-400';
    case 'execution':
      return 'text-purple-400';
    case 'system':
      return 'text-cyan-400';
    case 'graph':
      return 'text-orange-400';
    case 'data':
      return 'text-pink-400';
    default:
      return 'text-muted-foreground';
  }
};

interface LogDetailPanelProps {
  log: {
    timestamp: string;
    level: LogLevel;
    log_type: LogType;
    source?: string;
    message: string;
  };
}

export function LogDetailPanel({ log }: LogDetailPanelProps) {
  const { t } = useTranslation();
  const logTypeLabel =
    t(`detail.log.types.${log.log_type}`, { defaultValue: log.log_type.toUpperCase() });

  return (
    <DetailPanelShell title={t('detail.titleLog')}>
      <DetailForm>
        <DetailReadonlyField label={t('detail.fields.time')} tone="mono" className="text-foreground">
          {log.timestamp}
        </DetailReadonlyField>
        <DetailFieldRow label={t('detail.fields.level')}>
          <div className="flex min-h-8 items-center">
            <DetailBadge className={getLevelColor(log.level)}>{log.level}</DetailBadge>
          </div>
        </DetailFieldRow>
        <DetailFieldRow label={t('detail.fields.type')}>
          <div className="flex min-h-8 items-center">
            <DetailBadge className={getTypeColor(log.log_type)}>{logTypeLabel}</DetailBadge>
          </div>
        </DetailFieldRow>
        {log.source && (
          <DetailReadonlyField label={t('detail.fields.source')} tone="mono" className="text-cyan-400">
            {log.source}
          </DetailReadonlyField>
        )}
      </DetailForm>
      <DetailCollapsibleSection title={t('detail.fields.message')} defaultOpen>
        <DetailText
          as="pre"
          tone="mono"
          className="min-h-20 overflow-x-auto whitespace-pre-wrap break-words px-1 py-2 text-foreground"
        >
          {log.message}
        </DetailText>
      </DetailCollapsibleSection>
    </DetailPanelShell>
  );
}
