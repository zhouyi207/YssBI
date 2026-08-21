import { useTranslation } from 'react-i18next';
import type { DatabaseRecord } from '@/shared/types/dto/database';
import { databaseSourcePath } from '@/shared/types/dto/database';
import { DetailPanelShell } from '../shared/DetailPanelShell';
import { DetailCollapsibleSection } from '../shared/DetailCollapsibleSection';
import { DetailColumnList } from '../shared/DetailColumnList';
import { DetailForm, DetailReadonlyField } from '../shared/DetailForm';

interface DataDetailPanelProps {
  dataframe: DatabaseRecord;
}

export function DataDetailPanel({ dataframe }: DataDetailPanelProps) {
  const { t } = useTranslation();
  const columnCount = dataframe.columnCount ?? dataframe.columns?.length ?? 0;
  const rowCount = dataframe.rowCount ?? 0;
  const sourcePath = databaseSourcePath(dataframe.engine);

  return (
    <DetailPanelShell>
      <DetailForm>
        <DetailReadonlyField label={t('detail.fields.name')} tone="body">
          {dataframe.name}
        </DetailReadonlyField>
        <DetailReadonlyField label={t('detail.fields.columns')}>
          {t('detail.counts.columns', { count: columnCount })}
        </DetailReadonlyField>
        <DetailReadonlyField label={t('detail.fields.rows')}>
          {t('detail.counts.rows', { count: rowCount })}
        </DetailReadonlyField>
        {sourcePath && (
          <DetailReadonlyField label={t('detail.fields.source')} tone="mono" valueClassName="break-all">
            {sourcePath}
          </DetailReadonlyField>
        )}
      </DetailForm>
      {dataframe.columns && dataframe.columns.length > 0 && (
        <DetailCollapsibleSection title={t('detail.fields.columns')}>
          <DetailColumnList columns={dataframe.columns} />
        </DetailCollapsibleSection>
      )}
    </DetailPanelShell>
  );
}
