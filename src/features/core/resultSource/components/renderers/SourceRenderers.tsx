import type { ReactNode } from 'react';
import { OverlayScrollbar } from '@/shared/ui/OverlayScrollbar';
import type { SourceDescriptor } from '../../types';
import type { SourceViewLayout } from '../SourceViewShell';
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
  layout,
}: {
  payload: SourceDescriptor;
  meta?: ReactNode;
  layout: SourceViewLayout;
}) {
  const totalRows = payload.totalRows ?? payload.length ?? 0;
  const columns = payload.columns ?? [];
  const paging = usePagedSourceRows(payload.sourceId, totalRows);
  const fillHeight = layout === 'window';

  return (
    <SourceViewShell
      title={payload.title}
      meta={meta}
      layout={layout}
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
        <div className={fillHeight ? 'flex min-h-0 flex-1 flex-col' : undefined}>
          <ReadOnlyDataGrid
            columns={columns.map((name) => ({ name }))}
            rows={paging.rows}
            pageStartIndex={paging.offset}
            loading={paging.loading}
            height={fillHeight ? '100%' : 480}
            fillHeight={fillHeight}
          />
        </div>
      )}
    </SourceViewShell>
  );
}

export function DataFrameSourceView({
  payload,
  layout = 'embedded',
}: {
  payload: SourceDescriptor;
  layout?: SourceViewLayout;
}) {
  const totalRows = payload.totalRows ?? 0;
  const columns = payload.columns ?? [];

  return (
    <TabularSourceView
      payload={payload}
      layout={layout}
      meta={
        <span>
          {totalRows} rows × {columns.length} columns
        </span>
      }
    />
  );
}

export function DataSeriesSourceView({
  payload,
  layout = 'embedded',
}: {
  payload: SourceDescriptor;
  layout?: SourceViewLayout;
}) {
  const length = payload.totalRows ?? payload.length ?? 0;

  return (
    <TabularSourceView
      payload={payload}
      layout={layout}
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

export function ScalarSourceView({
  payload,
  layout = 'embedded',
}: {
  payload: SourceDescriptor;
  layout?: SourceViewLayout;
}) {
  const { value, loading, error } = useSourceValue(payload.sourceId);
  const isWindow = layout === 'window';

  return (
    <SourceViewShell title={payload.title} layout={layout}>
      {error ? (
        <SourceError error={error} />
      ) : (
        <div
          className={[
            'rounded-lg border border-border bg-card p-4 font-mono text-sm',
            isWindow ? 'flex min-h-0 flex-1 flex-col' : '',
          ].join(' ')}
        >
          <div className="mb-1 shrink-0 text-xs text-muted-foreground">
            {value?.valueType ?? payload.valueType ?? 'Value'}
          </div>
          {isWindow ? (
            <OverlayScrollbar className="min-h-0 flex-1">
              <pre className="break-all text-[var(--accent-color)]">
                {loading ? 'Loading…' : JSON.stringify(value?.value, null, 2)}
              </pre>
            </OverlayScrollbar>
          ) : (
            <pre className="break-all text-[var(--accent-color)]">
              {loading ? 'Loading…' : JSON.stringify(value?.value, null, 2)}
            </pre>
          )}
        </div>
      )}
    </SourceViewShell>
  );
}

export function JsonSourceView({
  payload,
  layout = 'embedded',
}: {
  payload: SourceDescriptor;
  layout?: SourceViewLayout;
}) {
  const { value, loading, error } = useSourceValue(payload.sourceId);
  const isWindow = layout === 'window';
  const jsonValue = value?.value ?? value;

  return (
    <SourceViewShell
      title={payload.title}
      layout={layout}
      meta={
        payload.typeKey ? (
          <span>
            Type: {payload.typeKey}
            {payload.handleId ? ` · Handle: ${payload.handleId}` : null}
          </span>
        ) : (
          (value?.valueType ?? payload.valueType) && (
            <span>Type: {value?.valueType ?? payload.valueType}</span>
          )
        )
      }
    >
      {error ? (
        <SourceError error={error} />
      ) : loading ? (
        <p className="text-sm text-muted-foreground">Loading…</p>
      ) : isWindow ? (
        <OverlayScrollbar className="min-h-0 flex-1">
          <JsonTreeView value={jsonValue} />
        </OverlayScrollbar>
      ) : (
        <JsonTreeView value={jsonValue} />
      )}
    </SourceViewShell>
  );
}

export function NullSourceView({
  payload,
  layout = 'embedded',
}: {
  payload: SourceDescriptor;
  layout?: SourceViewLayout;
}) {
  const { value, error } = useSourceValue(payload.sourceId);

  return (
    <SourceViewShell title={payload.title} layout={layout}>
      {error ? (
        <SourceError error={error} />
      ) : (
        <p className="text-muted-foreground">{value?.message ?? payload.message ?? 'No data'}</p>
      )}
    </SourceViewShell>
  );
}
