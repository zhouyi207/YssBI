import { Button } from "@/components/ui/button";

interface ResultPageToolbarProps {
  pageIndex: number;
  totalPages: number | null;
  totalCount: number | null;
  actualCount: number;
  hasMore: boolean;
  pageSize: number;
  loading?: boolean;
  onPrevious: () => void;
  onNext: () => void;
}

export function ResultPageToolbar({
  pageIndex,
  totalPages,
  totalCount,
  actualCount,
  hasMore,
  pageSize,
  loading,
  onPrevious,
  onNext,
}: ResultPageToolbarProps) {
  const start = actualCount === 0 ? 0 : pageIndex * pageSize + 1;
  const end = pageIndex * pageSize + actualCount;

  return (
    <div className="flex items-center gap-2 text-xs text-muted-foreground">
      <span>
        {actualCount === 0
          ? "0 rows"
          : totalCount === null
            ? `${start}–${end}`
            : `${start}–${end} of ${totalCount}`}
      </span>
      <Button
        type="button"
        variant="outline"
        size="sm"
        disabled={loading || pageIndex <= 0}
        onClick={onPrevious}
      >
        Prev
      </Button>
      <span>
        {totalPages === null ? `Page ${pageIndex + 1}` : `${pageIndex + 1} / ${totalPages}`}
      </span>
      <Button
        type="button"
        variant="outline"
        size="sm"
        disabled={loading || !hasMore}
        onClick={onNext}
      >
        Next
      </Button>
    </div>
  );
}
