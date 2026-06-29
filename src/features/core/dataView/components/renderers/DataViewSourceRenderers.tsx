import { TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table';
import { InfoStatsTable } from '@/views/InfoView/shared/InfoStatsTable';
import { OLSComponent } from '@/views/InfoView/OLSComponent';
import type { OLSResultData } from '@/views/InfoView/shared/types';
import type { SourceDescriptor } from '../../types';
import { useDataViewSourceValue } from '../../useDataViewSourceValue';
import { usePagedDataViewRows } from '../../usePagedDataViewRows';
import { DataViewPageToolbar } from '../DataViewPageToolbar';
import { DataViewShell } from '../DataViewShell';
import { ReadOnlyDataGrid } from '../ReadOnlyDataGrid';

function formatValue(value: unknown): string {
  if (value === null || value === undefined) return '—';
  if (typeof value === 'object') return JSON.stringify(value);
  return String(value);
}

function SourceError({ error }: { error: string }) {
  return <p className="text-sm text-destructive">{error}</p>;
}

export function DataFrameSourceView({ payload }: { payload: SourceDescriptor }) {
  const totalRows = payload.totalRows ?? 0;
  const columns = payload.columns ?? [];
  const paging = usePagedDataViewRows(payload.sourceId, totalRows);

  return (
    <DataViewShell
      title={payload.title}
      meta={
        <span>
          {totalRows} rows × {columns.length} columns
        </span>
      }
      toolbar={
        <DataViewPageToolbar
          pageIndex={paging.pageIndex}
          totalPages={paging.totalPages}
          totalCount={paging.totalCount || totalRows}
          pageSize={paging.pageSize}
          loading={paging.loading}
          onPrevious={paging.goToPreviousPage}
          onNext={paging.goToNextPage}
        />
      }
    >
      {paging.error ? (
        <SourceError error={paging.error} />
      ) : (
        <ReadOnlyDataGrid
          columns={columns.map((name) => ({ name }))}
          rows={paging.rows}
          pageStartIndex={paging.offset}
          loading={paging.loading}
          height={480}
        />
      )}
    </DataViewShell>
  );
}

export function SeriesSourceView({ payload }: { payload: SourceDescriptor }) {
  const length = payload.length ?? 0;
  const paging = usePagedDataViewRows(payload.sourceId, length);

  return (
    <DataViewShell
      title={payload.title}
      meta={
        <span>
          {payload.name ? `Name: ${payload.name} · ` : null}
          {payload.dtype ? `Type: ${payload.dtype} · ` : null}
          Length: {length}
        </span>
      }
      toolbar={
        <DataViewPageToolbar
          pageIndex={paging.pageIndex}
          totalPages={paging.totalPages}
          totalCount={paging.totalCount || length}
          pageSize={paging.pageSize}
          loading={paging.loading}
          onPrevious={paging.goToPreviousPage}
          onNext={paging.goToNextPage}
        />
      }
    >
      {paging.error ? (
        <SourceError error={paging.error} />
      ) : (
        <InfoStatsTable className="bg-card" tableClassName="text-sm">
          <TableHeader>
            <TableRow className="border-b border-border hover:bg-transparent">
              <TableHead className="h-auto px-4 py-2 text-left font-medium text-muted-foreground">#</TableHead>
              <TableHead className="h-auto px-4 py-2 text-left font-medium text-muted-foreground">Value</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {paging.values.map((v, i) => (
              <TableRow key={i} className="border-b border-border hover:bg-muted/50">
                <TableCell className="px-4 py-2 text-muted-foreground">{paging.offset + i}</TableCell>
                <TableCell className="px-4 py-2 font-mono text-foreground">{formatValue(v)}</TableCell>
              </TableRow>
            ))}
          </TableBody>
        </InfoStatsTable>
      )}
    </DataViewShell>
  );
}

export function ScalarSourceView({ payload }: { payload: SourceDescriptor }) {
  const { value, loading, error } = useDataViewSourceValue(payload.sourceId);

  return (
    <DataViewShell title={payload.title}>
      {error ? (
        <SourceError error={error} />
      ) : (
        <div className="rounded-lg border border-border bg-card p-4 font-mono text-sm">
          <div className="mb-1 text-xs text-muted-foreground">
            {value?.valueType ?? payload.valueType ?? 'Value'}
          </div>
          <pre className="break-all text-[var(--accent-color)]">
            {loading ? 'Loading…' : JSON.stringify(value?.value, null, 2)}
          </pre>
        </div>
      )}
    </DataViewShell>
  );
}

export function NullSourceView({ payload }: { payload: SourceDescriptor }) {
  const { value, error } = useDataViewSourceValue(payload.sourceId);

  return (
    <DataViewShell title={payload.title}>
      {error ? (
        <SourceError error={error} />
      ) : (
        <p className="text-muted-foreground">{value?.message ?? payload.message ?? 'No data'}</p>
      )}
    </DataViewShell>
  );
}

export function GenericStructSourceView({ payload }: { payload: SourceDescriptor }) {
  const { value, error } = useDataViewSourceValue(payload.sourceId);

  return (
    <DataViewShell
      title={payload.title}
      meta={payload.typeKey ? <span>Type: {payload.typeKey}</span> : null}
    >
      {error ? (
        <SourceError error={error} />
      ) : (
        <>
          <p className="text-muted-foreground">
            {value?.message ?? payload.message ?? 'Struct preview is not available for this type.'}
          </p>
          {payload.handleId ? (
            <p className="mt-2 text-xs text-muted-foreground">Handle: {payload.handleId}</p>
          ) : null}
        </>
      )}
    </DataViewShell>
  );
}

export function OlsStructSourceView({ payload }: { payload: SourceDescriptor }) {
  const { value, loading, error } = useDataViewSourceValue(payload.sourceId);

  if (error) {
    return (
      <DataViewShell title={payload.title}>
        <SourceError error={error} />
      </DataViewShell>
    );
  }

  if (loading || !value?.structured) {
    return (
      <DataViewShell title={payload.title}>
        <p className="text-muted-foreground">Loading…</p>
      </DataViewShell>
    );
  }

  return <OLSComponent data={value.structured as OLSResultData} />;
}
