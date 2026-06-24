import React from 'react';

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
      <div className="p-6 max-w-[900px] mx-auto">
        <h1 className="text-xl font-bold text-foreground mb-4">{title}</h1>
        <p className="text-muted-foreground">{data.message ?? 'No data'}</p>
      </div>
    );
  }

  if (dataType === 'scalar') {
    return (
      <div className="p-6 max-w-[900px] mx-auto">
        <h1 className="text-xl font-bold text-foreground mb-4">{title}</h1>
        <div className="rounded-lg border border-border bg-card p-4 font-mono text-sm">
          <div className="text-muted-foreground text-xs mb-1">{data.valueType ?? 'Value'}</div>
          <pre className="text-[var(--accent-color)] break-all">
            {JSON.stringify(data.value, null, 2)}
          </pre>
        </div>
      </div>
    );
  }

  if (dataType === 'series') {
    const values = data.values ?? [];
    const length = data.length ?? 0;
    const previewCount = data.previewCount ?? values.length;

    return (
      <div className="p-6 max-w-[900px] mx-auto">
        <h1 className="text-xl font-bold text-foreground mb-4">{title}</h1>
        <div className="text-muted-foreground text-sm mb-2">
          {data.name && <span className="mr-3">Name: {data.name}</span>}
          {data.dtype && <span className="mr-3">Type: {data.dtype}</span>}
          <span>Length: {length}</span>
          {length > previewCount && (
            <span className="ml-2 text-amber-500">(showing first {previewCount})</span>
          )}
        </div>
        <div className="rounded-lg border border-border bg-card overflow-x-auto">
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b border-border">
                <th className="px-4 py-2 text-left text-muted-foreground font-medium">#</th>
                <th className="px-4 py-2 text-left text-muted-foreground font-medium">Value</th>
              </tr>
            </thead>
            <tbody>
              {values.map((v, i) => (
                <tr key={i} className="border-b border-border hover:bg-muted/50">
                  <td className="px-4 py-2 text-muted-foreground">{i}</td>
                  <td className="px-4 py-2 text-foreground font-mono">
                    {typeof v === 'object' ? JSON.stringify(v) : String(v)}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>
    );
  }

  if (dataType === 'dataframe') {
    const columns = data.columns ?? [];
    const rows = data.rows ?? [];
    const totalRows = data.totalRows ?? 0;
    const previewRows = data.previewRows ?? rows.length;

    return (
      <div className="p-6 max-w-[1200px] mx-auto">
        <h1 className="text-xl font-bold text-foreground mb-4">{title}</h1>
        <div className="text-muted-foreground text-sm mb-2">
          <span>{totalRows} rows × {columns.length} columns</span>
          {totalRows > previewRows && (
            <span className="ml-2 text-amber-500">(showing first {previewRows} rows)</span>
          )}
        </div>
        <div className="rounded-lg border border-border bg-card overflow-x-auto">
          <table className="w-full text-sm min-w-[400px]">
            <thead>
              <tr className="border-b border-border">
                <th className="px-4 py-2 text-left text-muted-foreground font-medium sticky left-0 bg-card">#</th>
                {columns.map((col, i) => (
                  <th key={i} className="px-4 py-2 text-left text-muted-foreground font-medium whitespace-nowrap">
                    {col}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {rows.map((row, rowIdx) => (
                <tr key={rowIdx} className="border-b border-border hover:bg-muted/50">
                  <td className="px-4 py-2 text-muted-foreground sticky left-0 bg-card">{rowIdx}</td>
                  {(row as unknown[]).map((cell, colIdx) => (
                    <td key={colIdx} className="px-4 py-2 text-foreground font-mono whitespace-nowrap">
                      {cell === null || cell === undefined
                        ? '—'
                        : typeof cell === 'object'
                          ? JSON.stringify(cell)
                          : String(cell)}
                    </td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>
    );
  }

  return (
    <div className="p-6 max-w-[900px] mx-auto">
      <h1 className="text-xl font-bold text-foreground mb-4">{title}</h1>
      <p className="text-muted-foreground">Unknown data type: {dataType}</p>
    </div>
  );
};
