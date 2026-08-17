import { ScrollArea } from '@/components/ui/scroll-area';
import type { ResultDescriptor } from '../../types';
import { useResultValue } from '../../useResultValue';
import { usePagedResultRows } from '../../usePagedResultRows';
import { JsonTreeView } from '../JsonTreeView';
import { ResultPageToolbar } from '../ResultPageToolbar';
import { ResultViewShell } from '../ResultViewShell';
import { ReadOnlyDataGrid } from '../ReadOnlyDataGrid';
import { ResultReadError } from '../ResultReadError';

export function SequenceResultView({ payload }: { payload: ResultDescriptor }) {
  const totalCount = payload.totalCount ?? 0;
  const paging = usePagedResultRows(payload.resultId, totalCount);
  return (
    <ResultViewShell
      title={payload.title}
      toolbar={<ResultPageToolbar
        pageIndex={paging.pageIndex}
        totalPages={paging.totalPages}
        totalCount={paging.totalCount || totalCount}
        pageSize={paging.pageSize}
        loading={paging.loading}
        onPrevious={paging.goToPreviousPage}
        onNext={paging.goToNextPage}
      />}
    >
      {paging.error ? <ResultReadError error={paging.error} /> : (
        <ReadOnlyDataGrid
          columns={[]}
          rows={paging.rows}
          pageStartIndex={paging.offset}
          loading={paging.loading}
          height="100%"
          fillHeight
        />
      )}
    </ResultViewShell>
  );
}

export function DataSeriesResultView({ payload }: { payload: ResultDescriptor }) {
  const totalCount = payload.totalCount ?? payload.metadata?.length ?? 0;
  const paging = usePagedResultRows(payload.resultId, totalCount);
  return (
    <ResultViewShell title={payload.title} meta={<span>Length: {totalCount}</span>}>
      {paging.error ? <ResultReadError error={paging.error} /> : (
        <ScrollArea className="min-h-0 flex-1">
          <JsonTreeView value={paging.values} />
        </ScrollArea>
      )}
    </ResultViewShell>
  );
}

export function ScalarResultView({ payload }: { payload: ResultDescriptor }) {
  const { value, loading, error } = useResultValue(payload.resultId);
  return (
    <ResultViewShell title={payload.title}>
      {error ? <ResultReadError error={error} /> : (
        <ScrollArea className="min-h-0 flex-1">
          <pre className="break-all text-sm">{loading ? 'Loading…' : JSON.stringify(value?.value, null, 2)}</pre>
        </ScrollArea>
      )}
    </ResultViewShell>
  );
}

export function JsonResultView({ payload }: { payload: ResultDescriptor }) {
  const { value, loading, error } = useResultValue(payload.resultId);
  return (
    <ResultViewShell title={payload.title}>
      {error ? <ResultReadError error={error} /> : loading ? (
        <p className="text-sm text-muted-foreground">Loading…</p>
      ) : (
        <ScrollArea className="min-h-0 flex-1">
          <JsonTreeView value={value?.value ?? value} />
        </ScrollArea>
      )}
    </ResultViewShell>
  );
}
