import { useEffect, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { Input } from '@/components/ui/input';
import { Table, TableBody } from '@/components/ui/table';
import { useEditorGroup } from '@/features/application/editor';
import { performWorksheetDelete } from '@/features/application/editor/closeEditorTab';
import { useWorksheetStore } from '@/features/core/worksheet/worksheetStore';
import { DatabaseService } from '@/services/database/databaseService';
import { useDatabaseStore } from '@/features/core/dataStore/databaseStore';
import { Select } from '@/shared/ui';
import type { WorksheetChartType, WorksheetDocument } from '@/shared/types/domain/worksheet';
import { DetailPanelShell } from '../shared/DetailPanelShell';
import { DetailDeleteButton } from '../shared/DetailDeleteButton';
import { DetailFieldRow } from '../shared/DetailFieldRow';
import {
  detailInlineInputClass,
  detailListItemClass,
  detailSubsectionTitleClass,
  detailTableClass,
  detailValueMutedClass,
} from '../shared/detailStyles';

const CHART_TYPES: WorksheetChartType[] = ['histogram', 'scatter', 'line'];

function isNumericType(type: string): boolean {
  const t = type.toLowerCase();
  return (
    t.includes('int') ||
    t.includes('float') ||
    t.includes('double') ||
    t.includes('decimal') ||
    t.includes('number') ||
    t.includes('date')
  );
}

interface WorksheetDetailPanelProps {
  document: WorksheetDocument;
  onDeleted: () => void;
}

export function WorksheetDetailPanel({ document, onDeleted }: WorksheetDetailPanelProps) {
  const { t } = useTranslation();
  const { dataframes } = useEditorGroup();
  const updateDocument = useWorksheetStore((s) => s.updateDocument);
  const updateDatabase = useDatabaseStore((s) => s.updateDatabase);
  const databases = dataframes ?? {};

  const databaseOptions = useMemo(
    () =>
      Object.values(databases).map((db) => ({
        label: (db as { name?: string }).name ?? (db as { id: string }).id,
        value: (db as { id: string }).id,
      })),
    [databases],
  );

  const columns = useMemo(() => {
    const db = document.databaseId
      ? (databases[document.databaseId] as { columns?: Array<{ name: string; type: string }> } | undefined)
      : undefined;
    return db?.columns ?? [];
  }, [document.databaseId, databases]);

  useEffect(() => {
    const databaseId = document.databaseId;
    if (!databaseId) return;
    const existing = databases[databaseId] as { columns?: unknown[] } | undefined;
    if (existing?.columns && existing.columns.length > 0) return;
    void DatabaseService.getDatabaseMeta(databaseId).then((meta) => {
      updateDatabase(databaseId, {
        name: meta.name,
        columns: meta.columns,
        rowCount: meta.rowCount,
        columnCount: meta.columnCount,
      });
    });
  }, [document.databaseId, databases, updateDatabase]);

  const numericColumns = columns.filter((c) => isNumericType(c.type));
  const allColumnOptions = columns.map((c) => ({ label: c.name, value: c.name }));
  const numericColumnOptions = numericColumns.map((c) => ({ label: c.name, value: c.name }));

  const patch = (changes: Parameters<typeof updateDocument>[1]) => {
    updateDocument(document.id, changes);
  };

  const encodingLabelClass = 'align-top pt-2';

  return (
    <DetailPanelShell title={t('detail.titleWithName', { name: document.name })}>
      <Table className={detailTableClass}>
        <TableBody>
          <DetailFieldRow label={t('detail.fields.name')} labelWidth="wide">
            <Input
              className={detailInlineInputClass}
              value={document.name}
              onChange={(e) => patch({ name: e.target.value })}
            />
          </DetailFieldRow>
          <DetailFieldRow
            label={t('chartsSidebar.dataset')}
            labelWidth="wide"
            labelClassName={encodingLabelClass}
          >
            <Select
              value={document.databaseId}
              options={databaseOptions}
              onChange={(val) => patch({ databaseId: val, encodings: {} })}
            />
          </DetailFieldRow>
          <DetailFieldRow
            label={t('chartsSidebar.chartType')}
            labelWidth="wide"
            labelClassName={encodingLabelClass}
          >
            <Select
              value={document.chartType}
              options={CHART_TYPES.map((type) => ({
                value: type,
                label: t(`chartsSidebar.chartTypes.${type}`),
              }))}
              onChange={(val) => patch({ chartType: val as WorksheetChartType, encodings: {} })}
            />
          </DetailFieldRow>
          {document.chartType === 'histogram' ? (
            <DetailFieldRow
              label={t('chartsSidebar.encodingY')}
              labelWidth="wide"
              labelClassName={encodingLabelClass}
            >
              <Select
                value={document.encodings.y ?? ''}
                options={allColumnOptions}
                onChange={(val) => patch({ encodings: { ...document.encodings, y: val } })}
              />
            </DetailFieldRow>
          ) : (
            <>
              <DetailFieldRow
                label={t('chartsSidebar.encodingX')}
                labelWidth="wide"
                labelClassName={encodingLabelClass}
              >
                <Select
                  value={document.encodings.x ?? ''}
                  options={numericColumnOptions}
                  onChange={(val) => patch({ encodings: { ...document.encodings, x: val } })}
                />
              </DetailFieldRow>
              <DetailFieldRow
                label={t('chartsSidebar.encodingY')}
                labelWidth="wide"
                labelClassName={encodingLabelClass}
              >
                <Select
                  value={document.encodings.y ?? ''}
                  options={numericColumnOptions}
                  onChange={(val) => patch({ encodings: { ...document.encodings, y: val } })}
                />
              </DetailFieldRow>
            </>
          )}
        </TableBody>
      </Table>

      <div className="px-2 pt-3">
        <div className={`mb-1 ${detailSubsectionTitleClass}`}>{t('chartsSidebar.columns')}</div>
        <div className="space-y-0.5">
          {columns.map((col) => (
            <div key={col.name} className={detailListItemClass}>
              <span className="truncate">{col.name}</span>
              <span className="ml-2 shrink-0 text-[10px] text-[var(--accent-color)]/70">{col.type}</span>
            </div>
          ))}
          {columns.length === 0 && (
            <div className={`px-2 text-[11px] ${detailValueMutedClass}`}>
              {t('chartsSidebar.noColumns')}
            </div>
          )}
        </div>
      </div>

      <DetailDeleteButton
        itemType="worksheet"
        itemName={document.name}
        onDelete={async () => {
          await performWorksheetDelete(document.id);
        }}
        onDeleted={onDeleted}
      />
    </DetailPanelShell>
  );
}
