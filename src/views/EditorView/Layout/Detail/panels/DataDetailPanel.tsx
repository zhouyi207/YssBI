import { useTranslation } from 'react-i18next';
import { Input } from '@/components/ui/input';
import { OverlayScrollbar } from '@/shared/ui/OverlayScrollbar';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table';
import { DetailPanelShell } from '../shared/DetailPanelShell';
import { DetailDeleteButton } from '../shared/DetailDeleteButton';
import { DetailFieldRow } from '../shared/DetailFieldRow';
import {
  detailInlineInputClass,
  detailNestedScrollClass,
  detailTableClass,
  detailValueMutedClass,
} from '../shared/detailStyles';

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
  onDelete: () => Promise<void>;
}

export function DataDetailPanel({ dataframe, onUpdate, onDelete }: DataDetailPanelProps) {
  const { t } = useTranslation();
  const columnCount = dataframe.columnCount || dataframe.columns?.length || 0;
  const rowCount = dataframe.rowCount || dataframe.rows?.length || 0;

  return (
    <DetailPanelShell title={t('detail.titleWithName', { name: dataframe.name })}>
      <Table className={detailTableClass}>
        <TableBody>
          <DetailFieldRow label={t('detail.fields.name')}>
            <Input
              className={detailInlineInputClass}
              value={dataframe.name}
              onChange={(e) => onUpdate({ name: e.target.value })}
            />
          </DetailFieldRow>
          <DetailFieldRow label={t('detail.fields.columns')} valueClassName={detailValueMutedClass}>
            {t('detail.counts.columns', { count: columnCount })}
          </DetailFieldRow>
          {dataframe.columns && dataframe.columns.length > 0 && (
            <TableRow>
              <TableCell colSpan={2} className="p-0">
                <OverlayScrollbar className={detailNestedScrollClass} direction="vertical">
                  <Table className="text-[9px]">
                    <TableHeader>
                      <TableRow className="text-muted-foreground">
                        <TableHead className="h-6 p-1 font-normal uppercase">
                          {t('detail.fields.column')}
                        </TableHead>
                        <TableHead className="h-6 p-1 font-normal uppercase">
                          {t('detail.fields.type')}
                        </TableHead>
                      </TableRow>
                    </TableHeader>
                    <TableBody>
                      {dataframe.columns.map((col) => (
                        <TableRow key={col.name} className="border-border/50">
                          <TableCell className="p-1 font-medium text-foreground">{col.name}</TableCell>
                          <TableCell className="p-1 text-[var(--accent-color)]/70">{col.type}</TableCell>
                        </TableRow>
                      ))}
                    </TableBody>
                  </Table>
                </OverlayScrollbar>
              </TableCell>
            </TableRow>
          )}
          <DetailFieldRow label={t('detail.fields.rows')} valueClassName={detailValueMutedClass}>
            {t('detail.counts.rows', { count: rowCount })}
          </DetailFieldRow>
          {dataframe.sourcePath && (
            <DetailFieldRow
              label={t('detail.fields.source')}
              valueClassName={`break-all text-[9px] ${detailValueMutedClass}`}
            >
              {dataframe.sourcePath}
            </DetailFieldRow>
          )}
        </TableBody>
      </Table>
      <DetailDeleteButton itemType="data" itemName={dataframe.name} onDelete={onDelete} />
    </DetailPanelShell>
  );
}
