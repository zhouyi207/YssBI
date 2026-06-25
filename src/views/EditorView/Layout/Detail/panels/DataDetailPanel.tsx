import { Input } from '@/components/ui/input';
import { OverlayScrollbar } from '@/shared/ui/OverlayScrollbar';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table';
import { DetailPanelShell } from '../shared/DetailPanelShell';
import { DetailDeleteButton } from '../shared/DetailDeleteButton';

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
  return (
    <DetailPanelShell title={`Details : ${dataframe.name}`}>
      <Table className="text-[11px] text-[#cccccc]">
        <TableBody>
          <TableRow>
            <TableCell className="w-20 bg-white/5 font-bold text-gray-400">Name</TableCell>
            <TableCell>
              <Input
                className="h-7 border-0 bg-transparent px-0 py-0 font-medium shadow-none"
                value={dataframe.name}
                onChange={(e) => onUpdate({ name: e.target.value })}
              />
            </TableCell>
          </TableRow>
          <TableRow>
            <TableCell className="bg-white/5 font-bold text-gray-400">Columns</TableCell>
            <TableCell className="text-gray-400">
              {dataframe.columnCount || dataframe.columns?.length || 0} columns
            </TableCell>
          </TableRow>
          {dataframe.columns && dataframe.columns.length > 0 && (
            <TableRow>
              <TableCell colSpan={2} className="p-0">
                <OverlayScrollbar className="max-h-40 bg-black/20" direction="vertical">
                  <Table className="text-[9px]">
                    <TableHeader>
                      <TableRow className="text-gray-500">
                        <TableHead className="h-6 p-1 font-normal uppercase">Column</TableHead>
                        <TableHead className="h-6 p-1 font-normal uppercase">Type</TableHead>
                      </TableRow>
                    </TableHeader>
                    <TableBody>
                      {dataframe.columns.map((col) => (
                        <TableRow key={col.name} className="border-white/5">
                          <TableCell className="p-1 font-medium text-gray-300">{col.name}</TableCell>
                          <TableCell className="p-1 text-[var(--accent-color)]/70">{col.type}</TableCell>
                        </TableRow>
                      ))}
                    </TableBody>
                  </Table>
                </OverlayScrollbar>
              </TableCell>
            </TableRow>
          )}
          <TableRow>
            <TableCell className="bg-white/5 font-bold text-gray-400">Rows</TableCell>
            <TableCell className="text-gray-400">
              {dataframe.rowCount || dataframe.rows?.length || 0} rows
            </TableCell>
          </TableRow>
          {dataframe.sourcePath && (
            <TableRow>
              <TableCell className="bg-white/5 font-bold text-gray-400">Source</TableCell>
              <TableCell className="break-all text-[9px] text-gray-400">{dataframe.sourcePath}</TableCell>
            </TableRow>
          )}
        </TableBody>
      </Table>
      <DetailDeleteButton itemType="data" itemName={dataframe.name} onDelete={onDelete} />
    </DetailPanelShell>
  );
}
