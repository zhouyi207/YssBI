import { useTranslation } from 'react-i18next';
import type { DiagnosticLevel, DiagnosticRecordDto } from '@/shared/types/dto/diagnostics';
import { DetailPanelShell } from '../shared/DetailPanelShell';
import { DetailForm, DetailReadonlyField } from '../shared/DetailForm';
import { DetailCollapsibleSection } from '../shared/DetailCollapsibleSection';
import { DetailFieldRow } from '../shared/DetailFieldRow';
import { DetailBadge, DetailText } from '../shared/DetailText';

const getLevelColor = (level: DiagnosticLevel) => {
  switch (level) {
    case 'error': return 'text-red-400';
    case 'warn': return 'text-yellow-400';
    case 'info': return 'text-blue-400';
    case 'debug': return 'text-muted-foreground';
    case 'trace': return 'text-muted-foreground/70';
  }
};

const getDomainColor = (domain: string) => {
  switch (domain) {
    case 'application': return 'text-green-400';
    case 'execution': return 'text-purple-400';
    case 'system': return 'text-cyan-400';
    case 'graph': return 'text-orange-400';
    case 'data': return 'text-pink-400';
    case 'ui': return 'text-amber-400';
    default: return 'text-muted-foreground';
  }
};

export function LogDetailPanel({ log }: { log: DiagnosticRecordDto }) {
  const { t } = useTranslation();
  const domainLabel = t(`detail.log.types.${log.domain}`, {
    defaultValue: log.domain.toUpperCase(),
  });
  const hasFields = Object.keys(log.fields).length > 0;

  return (
    <DetailPanelShell>
      <DetailForm className="select-text">
        <DetailReadonlyField label={t('detail.fields.time')} tone="mono" className="text-foreground">
          {log.timestamp}
        </DetailReadonlyField>
        <DetailReadonlyField
          label={t('detail.fields.stream', { defaultValue: 'Stream' })}
          tone="mono"
        >
          {log.streamId}
        </DetailReadonlyField>
        <DetailReadonlyField
          label={t('detail.fields.sequence', { defaultValue: 'Sequence' })}
          tone="mono"
        >
          {String(log.sequence)}
        </DetailReadonlyField>
        <DetailFieldRow label={t('detail.fields.level')}>
          <div className="flex min-h-8 items-center justify-end">
            <DetailBadge className={getLevelColor(log.level)}>{log.level}</DetailBadge>
          </div>
        </DetailFieldRow>
        <DetailFieldRow label={t('detail.fields.type')}>
          <div className="flex min-h-8 items-center justify-end">
            <DetailBadge className={getDomainColor(log.domain)}>{domainLabel}</DetailBadge>
          </div>
        </DetailFieldRow>
        <DetailReadonlyField
          label={t('detail.fields.origin', { defaultValue: 'Origin' })}
          tone="mono"
        >
          {log.origin}
        </DetailReadonlyField>
        <DetailReadonlyField
          label={t('detail.fields.target', { defaultValue: 'Target' })}
          tone="mono"
        >
          {log.target}
        </DetailReadonlyField>
        {log.event ? (
          <DetailReadonlyField
            label={t('detail.fields.event', { defaultValue: 'Event' })}
            tone="mono"
          >
            {log.event}
          </DetailReadonlyField>
        ) : null}
        {log.source ? (
          <DetailReadonlyField label={t('detail.fields.source')} tone="mono" className="text-cyan-400">
            {log.source}
          </DetailReadonlyField>
        ) : null}
      </DetailForm>
      <DetailCollapsibleSection
        title={t('detail.fields.message')}
        defaultOpen
        contentClassName="select-text"
      >
        <DetailText
          as="pre"
          tone="mono"
          className="min-h-20 overflow-x-auto whitespace-pre-wrap break-words px-1 py-2 text-foreground"
        >
          {log.message}
        </DetailText>
      </DetailCollapsibleSection>
      {hasFields ? (
        <DetailCollapsibleSection
          title={t('detail.fields.fields', { defaultValue: 'Fields' })}
          defaultOpen
          contentClassName="select-text"
        >
          <DetailText as="pre" tone="mono" className="overflow-x-auto whitespace-pre-wrap break-words px-1 py-2">
            {JSON.stringify(log.fields, null, 2)}
          </DetailText>
        </DetailCollapsibleSection>
      ) : null}
    </DetailPanelShell>
  );
}
