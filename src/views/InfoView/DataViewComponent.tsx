import React from 'react';
import { TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table';
import { InfoStatsTable } from './shared/InfoStatsTable';

export interface DataViewData {
  viewType: 'data_view';
  dataType: 'dataframe' | 'series' | 'scalar' | 'null' | 'struct';
  title: string;
  message?: string;
  columns?: string[];
  rows?: unknown[][];
  totalRows?: number;
  previewRows?: number;
  name?: string;
  dtype?: string;
  values?: unknown[];
  length?: number;
  previewCount?: number;
  value?: unknown;
  valueType?: string;
  typeKey?: string;
  handleId?: string;
}

export const DataViewComponent: React.FC<{ data: DataViewData }> = ({ data }) => {
  const { dataType, title } = data;

  if (dataType === 'null' || dataType === 'struct') {
    return (
      <div className="mx-auto max-w-[900px] p-6">
        <h1 className="mb-4 text-xl font-bold text-foreground">{title}</h1>
        <p className="text-muted-foreground">{data.message ?? 'No data'}</p>
      </div>
    );
  }

  if (dataType === 'scalar') {
    return (
      <div className="mx-auto max-w-[900px] p-6">
        <h1 className="mb-4 text-xl font-bold text-foreground">{title}</h1>
        <div className="rounded-lg border border-border bg-card p-4 font-mono text-sm">
          <div className="mb-1 text-xs text-muted-foreground">{data.valueType ?? 'Value'}</div>
          <pre className="break-all text-[var(--accent-color)]">{JSON.stringify(data.value, null, 2)}</pre>
        </div>
      </div>
    );
  }

  if (dataType === 'series') {
    const values = data.values ?? [];
    const length = data.length ?? 0;
    const previewCount = data.previewCount ?? values.length;

    return (
      <div className="mx-auto max-w-[900px] p-6">
        <h1 className="mb-4 text-xl font-bold text-foreground">{title}</h1>
        <div className="mb-2 text-sm text-muted-foreground">
          {data.name && <span className="mr-3">Name: {data.name}</span>}
          {data.dtype && <span className="mr-3">Type: {data.dtype}</span>}
          <span>Length: {length}</span>
          {length > previewCount && <span className="ml-2 text-amber-500">(showing first {previewCount})</span>}
        </div>
        <InfoStatsTable className="bg-card" tableClassName="text-sm">
          <TableHeader>
            <TableRow className="border-b border-border hover:bg-transparent">
              <TableHead className="h-auto px-4 py-2 text-left font-medium text-muted-foreground">#</TableHead>
              <TableHead className="h-auto px-4 py-2 text-left font-medium text-muted-foreground">Value</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {values.map((v, i) => (
              <TableRow key={i} className="border-b border-border hover:bg-muted/50">
                <TableCell className="px-4 py-2 text-muted-foreground">{i}</TableCell>
                <TableCell className="px-4 py-2 font-mono text-foreground">
                  {typeof v === 'object' ? JSON.stringify(v) : String(v)}
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </InfoStatsTable>
      </div>
    );
  }

  if (dataType === 'dataframe') {
    const columns = data.columns ?? [];
    const rows = data.rows ?? [];
    const totalRows = data.totalRows ?? 0;
    const previewRows = data.previewRows ?? rows.length;

    return (
      <div className="mx-auto max-w-[1200px] p-6">
        <h1 className="mb-4 text-xl font-bold text-foreground">{title}</h1>
        <div className="mb-2 text-sm text-muted-foreground">
          <span>
            {totalRows} rows × {columns.length} columns
          </span>
          {totalRows > previewRows && <span className="ml-2 text-amber-500">(showing first {previewRows} rows)</span>}
        </div>
        <InfoStatsTable className="overflow-x-auto bg-card" tableClassName="min-w-[400px] text-sm">
          <TableHeader>
            <TableRow className="border-b border-border hover:bg-transparent">
              <TableHead className="sticky left-0 h-auto bg-card px-4 py-2 text-left font-medium text-muted-foreground">#</TableHead>
              {columns.map((col, i) => (
                <TableHead key={i} className="h-auto whitespace-nowrap px-4 py-2 text-left font-medium text-muted-foreground">
                  {col}
                </TableHead>
              ))}
            </TableRow>
          </TableHeader>
          <TableBody>
            {rows.map((row, rowIdx) => (
              <TableRow key={rowIdx} className="border-b border-border hover:bg-muted/50">
                <TableCell className="sticky left-0 bg-card px-4 py-2 text-muted-foreground">{rowIdx}</TableCell>
                {(row as unknown[]).map((cell, colIdx) => (
                  <TableCell key={colIdx} className="whitespace-nowrap px-4 py-2 font-mono text-foreground">
                    {cell === null || cell === undefined
                      ? '—'
                      : typeof cell === 'object'
                        ? JSON.stringify(cell)
                        : String(cell)}
                  </TableCell>
                ))}
              </TableRow>
            ))}
          </TableBody>
        </InfoStatsTable>
      </div>
    );
  }

  return (
    <div className="mx-auto max-w-[900px] p-6">
      <h1 className="mb-4 text-xl font-bold text-foreground">{title}</h1>
      <p className="text-muted-foreground">Unknown data type: {dataType}</p>
    </div>
  );
};
