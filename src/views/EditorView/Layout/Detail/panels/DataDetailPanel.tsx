import { useTranslation } from 'react-i18next';
import { DetailPanelShell } from '../shared/DetailPanelShell';
import { DetailColumnList } from '../shared/DetailColumnList';
import { DetailForm, DetailNameField, DetailReadonlyField } from '../shared/DetailForm';

interface DataDetailPanelProps {
  dataframe: {
    id: string;
    name: string;
    columnCount?: number;
    columns?: Array<{ name: string; type: string }>;
    rowCount?: number;
    rows?: unknown[];
    sourcePath?: string;
  };
  onUpdate: (patch: Record<string, unknown>) => void;
}

export function DataDetailPanel({ dataframe, onUpdate }: DataDetailPanelProps) {
  const { t } = useTranslation();
  const columnCount = dataframe.columnCount || dataframe.columns?.length || 0;
  const rowCount = dataframe.rowCount || dataframe.rows?.length || 0;

  return (
    <DetailPanelShell title={t('detail.titleWithName', { name: dataframe.name })}>
      <DetailForm>
        <DetailNameField
          label={t('detail.fields.name')}
          value={dataframe.name}
          onCommit={(name) => onUpdate({ name })}
        />
        <DetailReadonlyField label={t('detail.fields.columns')}>
          {t('detail.counts.columns', { count: columnCount })}
        </DetailReadonlyField>
        {dataframe.columns && dataframe.columns.length > 0 && (
          <DetailColumnList
            columns={dataframe.columns}
            variant="table"
            columnLabel={t('detail.fields.column')}
            typeLabel={t('detail.fields.type')}
          />
        )}
        <DetailReadonlyField label={t('detail.fields.rows')}>
          {t('detail.counts.rows', { count: rowCount })}
        </DetailReadonlyField>
        {dataframe.sourcePath && (
          <DetailReadonlyField label={t('detail.fields.source')} tone="mono" valueClassName="break-all">
            {dataframe.sourcePath}
          </DetailReadonlyField>
        )}
      </DetailForm>
    </DetailPanelShell>
  );
}
