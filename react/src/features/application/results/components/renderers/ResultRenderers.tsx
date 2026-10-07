import { ScrollArea } from "@/components/ui/scroll-area";
import type { ResultDescriptor } from "../../types";
import { useResultValue } from "../../useResultValue";
import { usePagedResultRows } from "../../usePagedResultRows";
import { ResultPageToolbar } from "../ResultPageToolbar";
import { ResultViewShell } from "../ResultViewShell";
import { ReadOnlyDataGrid } from "../ReadOnlyDataGrid";
import { ResultReadError } from "../ResultReadError";

export function SequenceResultView({ payload }: { payload: ResultDescriptor }) {
  const totalCount = payload.totalCount;
  const paging = usePagedResultRows(payload, totalCount);
  return (
    <ResultViewShell
      title={payload.title}
      toolbar={
        <ResultPageToolbar
          pageIndex={paging.pageIndex}
          totalPages={paging.totalPages}
          totalCount={paging.totalCount}
          actualCount={paging.actualCount}
          hasMore={paging.hasMore}
          pageSize={paging.pageSize}
          loading={paging.loading}
          onPrevious={paging.goToPreviousPage}
          onNext={paging.goToNextPage}
        />
      }
    >
      {paging.error ? (
        <ResultReadError error={paging.error} />
      ) : (
        <ReadOnlyDataGrid
          columns={paging.columns}
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

export function ScalarResultView({ payload }: { payload: ResultDescriptor }) {
  const { value, loading, error } = useResultValue(payload);
  return (
    <ResultViewShell title={payload.title}>
      {error ? (
        <ResultReadError error={error} />
      ) : loading ? (
        <p className="text-sm">Loading…</p>
      ) : (
        <ResultJsonView value={value?.value} />
      )}
    </ResultViewShell>
  );
}

export function ResultJsonView({ value }: { value: unknown }) {
  return (
    <ScrollArea className="min-h-0 flex-1">
      <pre className="break-all text-sm">{JSON.stringify(value, null, 2)}</pre>
    </ScrollArea>
  );
}
