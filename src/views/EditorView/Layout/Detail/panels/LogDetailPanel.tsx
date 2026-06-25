import { useTranslation } from 'react-i18next';
import { Table, TableBody } from '@/components/ui/table';
import { LogLevel, LogType } from '@/shared/types/ui';
import { DetailPanelShell } from '../shared/DetailPanelShell';
import { DetailFieldRow } from '../shared/DetailFieldRow';
import { detailTableClass, detailValueMutedClass } from '../shared/detailStyles';

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
      <Table className={detailTableClass}>
        <TableBody>
          <DetailFieldRow label={t('detail.fields.time')} valueClassName="font-mono text-foreground">
            {log.timestamp}
          </DetailFieldRow>
          <DetailFieldRow label={t('detail.fields.level')}>
            <span className={`${getLevelColor(log.level)} font-bold uppercase`}>{log.level}</span>
          </DetailFieldRow>
          <DetailFieldRow label={t('detail.fields.type')}>
            <span className={`${getTypeColor(log.log_type)} font-semibold`}>{logTypeLabel}</span>
          </DetailFieldRow>
          {log.source && (
            <DetailFieldRow label={t('detail.fields.source')} valueClassName="font-mono text-cyan-400">
              {log.source}
            </DetailFieldRow>
          )}
          <DetailFieldRow
            label={t('detail.fields.message')}
            labelClassName="align-top"
            valueClassName="align-top"
          >
            <pre className="whitespace-pre-wrap break-all font-mono text-[11px] leading-relaxed text-foreground">
              {log.message}
            </pre>
          </DetailFieldRow>
        </TableBody>
      </Table>
    </DetailPanelShell>
  );
}
