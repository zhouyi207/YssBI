import { useEffect, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { Input } from '@/components/ui/input';
import { Table, TableBody, TableCell, TableRow } from '@/components/ui/table';
import { useEditorGroup } from '@/features/application/editor';
import { performWorksheetDelete } from '@/features/application/editor/closeEditorTab';
import { useWorksheetStore } from '@/features/core/worksheet/worksheetStore';
import { DatabaseService } from '@/services/database/databaseService';
import { useDatabaseStore } from '@/features/core/dataStore/databaseStore';
import { Select } from '@/shared/ui';
import type { WorksheetChartType, WorksheetDocument } from '@/shared/types/domain/worksheet';
import { DetailPanelShell } from '../shared/DetailPanelShell';
import { DetailDeleteButton } from '../shared/DetailDeleteButton';

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

  return (
    <DetailPanelShell title={t('detail.titleWithName', { name: document.name })}>
      <Table className="text-[11px] text-[#cccccc]">
        <TableBody>
          <TableRow>
            <TableCell className="w-24 bg-white/5 font-bold text-gray-400">
              {t('detail.fields.name')}
            </TableCell>
            <TableCell>
              <Input
                className="h-7 border-0 bg-transparent px-0 py-0 font-medium shadow-none"
                value={document.name}
                onChange={(e) => patch({ name: e.target.value })}
              />
            </TableCell>
          </TableRow>
          <TableRow>
            <TableCell className="bg-white/5 font-bold text-gray-400 align-top pt-2">
              {t('chartsSidebar.dataset')}
            </TableCell>
            <TableCell>
              <Select
                value={document.databaseId}
                options={databaseOptions}
                onChange={(val) => patch({ databaseId: val, encodings: {} })}
              />
            </TableCell>
          </TableRow>
          <TableRow>
            <TableCell className="bg-white/5 font-bold text-gray-400 align-top pt-2">
              {t('chartsSidebar.chartType')}
            </TableCell>
            <TableCell>
              <Select
                value={document.chartType}
                options={CHART_TYPES.map((type) => ({
                  value: type,
                  label: t(`chartsSidebar.chartTypes.${type}`),
                }))}
                onChange={(val) => patch({ chartType: val as WorksheetChartType, encodings: {} })}
              />
            </TableCell>
          </TableRow>
          {document.chartType === 'histogram' ? (
            <TableRow>
              <TableCell className="bg-white/5 font-bold text-gray-400 align-top pt-2">
                {t('chartsSidebar.encodingY')}
              </TableCell>
              <TableCell>
                <Select
                  value={document.encodings.y ?? ''}
                  options={allColumnOptions}
                  onChange={(val) => patch({ encodings: { ...document.encodings, y: val } })}
                />
              </TableCell>
            </TableRow>
          ) : (
            <>
              <TableRow>
                <TableCell className="bg-white/5 font-bold text-gray-400 align-top pt-2">
                  {t('chartsSidebar.encodingX')}
                </TableCell>
                <TableCell>
                  <Select
                    value={document.encodings.x ?? ''}
                    options={numericColumnOptions}
                    onChange={(val) => patch({ encodings: { ...document.encodings, x: val } })}
                  />
                </TableCell>
              </TableRow>
              <TableRow>
                <TableCell className="bg-white/5 font-bold text-gray-400 align-top pt-2">
                  {t('chartsSidebar.encodingY')}
                </TableCell>
                <TableCell>
                  <Select
                    value={document.encodings.y ?? ''}
                    options={numericColumnOptions}
                    onChange={(val) => patch({ encodings: { ...document.encodings, y: val } })}
                  />
                </TableCell>
              </TableRow>
            </>
          )}
        </TableBody>
      </Table>

      <div className="px-2 pt-3">
        <div className="mb-1 text-[10px] font-semibold uppercase text-gray-500">
          {t('chartsSidebar.columns')}
        </div>
        <div className="space-y-0.5">
          {columns.map((col) => (
            <div
              key={col.name}
              className="flex items-center justify-between rounded px-2 py-1 text-[11px] text-gray-400 hover:bg-white/5"
            >
              <span className="truncate">{col.name}</span>
              <span className="ml-2 shrink-0 text-[10px] text-[var(--accent-color)]/70">{col.type}</span>
            </div>
          ))}
          {columns.length === 0 && (
            <div className="px-2 text-[11px] text-gray-500/70">{t('chartsSidebar.noColumns')}</div>
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
