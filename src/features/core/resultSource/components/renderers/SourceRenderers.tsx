import type { ReactNode } from 'react';
import { OverlayScrollbar } from '@/shared/ui/OverlayScrollbar';
import type { SourceDescriptor } from '../../types';
import { useSourceValue } from '../../useSourceValue';
import { usePagedSourceRows } from '../../usePagedSourceRows';
import { JsonTreeView } from '../JsonTreeView';
import { SourcePageToolbar } from '../SourcePageToolbar';
import { SourceViewShell } from '../SourceViewShell';
import { ReadOnlyDataGrid } from '../ReadOnlyDataGrid';

function SourceError({ error }: { error: string }) {
  return <p className="text-sm text-destructive">{error}</p>;
}

function TabularSourceView({
  payload,
  meta,
}: {
  payload: SourceDescriptor;
  meta?: ReactNode;
}) {
  const totalRows = payload.totalRows ?? payload.length ?? 0;
  const columns = payload.columns ?? [];
  const paging = usePagedSourceRows(payload.sourceId, totalRows);

  return (
    <SourceViewShell
      title={payload.title}
      meta={meta}
      toolbar={
        <SourcePageToolbar
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
        <div className="flex min-h-0 flex-1 flex-col">
          <ReadOnlyDataGrid
            columns={columns.map((name) => ({ name }))}
            rows={paging.rows}
            pageStartIndex={paging.offset}
            loading={paging.loading}
            height="100%"
            fillHeight
          />
        </div>
      )}
    </SourceViewShell>
  );
}

export function DataFrameSourceView({ payload }: { payload: SourceDescriptor }) {
  const totalRows = payload.totalRows ?? 0;
  const columns = payload.columns ?? [];

  return (
    <TabularSourceView
      payload={payload}
      meta={
        <span>
          {totalRows} rows × {columns.length} columns
        </span>
      }
    />
  );
}

export function DataSeriesSourceView({ payload }: { payload: SourceDescriptor }) {
  const length = payload.totalRows ?? payload.length ?? 0;

  return (
    <TabularSourceView
      payload={payload}
      meta={
        <span>
          {payload.name ? `Name: ${payload.name} · ` : null}
          {payload.dtype ? `Type: ${payload.dtype} · ` : null}
          Length: {length}
        </span>
      }
    />
  );
}

export function ScalarSourceView({ payload }: { payload: SourceDescriptor }) {
  const { value, loading, error } = useSourceValue(payload.sourceId);

  return (
    <SourceViewShell title={payload.title}>
      {error ? (
        <SourceError error={error} />
      ) : (
        <div className="flex min-h-0 flex-1 flex-col rounded-lg border border-border bg-card p-4 font-mono text-sm">
          <div className="mb-1 shrink-0 text-xs text-muted-foreground">
            {payload.valueType ?? 'Value'}
          </div>
          <OverlayScrollbar className="min-h-0 flex-1">
            <pre className="break-all text-[var(--accent-color)]">
              {loading ? 'Loading…' : JSON.stringify(value?.value, null, 2)}
            </pre>
          </OverlayScrollbar>
        </div>
      )}
    </SourceViewShell>
  );
}

export function JsonSourceView({ payload }: { payload: SourceDescriptor }) {
  const { value, loading, error } = useSourceValue(payload.sourceId);
  const jsonValue = value?.value ?? value;

  return (
    <SourceViewShell
      title={payload.title}
      meta={
        payload.typeKey ? (
          <span>
            Type: {payload.typeKey}
            {payload.handleId ? ` · Handle: ${payload.handleId}` : null}
          </span>
        ) : (
          payload.valueType && <span>Type: {payload.valueType}</span>
        )
      }
    >
      {error ? (
        <SourceError error={error} />
      ) : loading ? (
        <p className="text-sm text-muted-foreground">Loading…</p>
      ) : (
        <OverlayScrollbar className="min-h-0 flex-1">
          <JsonTreeView value={jsonValue} />
        </OverlayScrollbar>
      )}
    </SourceViewShell>
  );
}

export function NullSourceView({ payload }: { payload: SourceDescriptor }) {
  const { error } = useSourceValue(payload.sourceId);

  return (
    <SourceViewShell title={payload.title}>
      {error ? (
        <SourceError error={error} />
      ) : (
        <p className="text-muted-foreground">{payload.message ?? 'No data'}</p>
      )}
    </SourceViewShell>
  );
}
