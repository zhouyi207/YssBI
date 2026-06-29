import { Button } from '@/components/ui/button';

interface DataViewPageToolbarProps {
  pageIndex: number;
  totalPages: number;
  totalCount: number;
  pageSize: number;
  loading?: boolean;
  onPrevious: () => void;
  onNext: () => void;
}

export function DataViewPageToolbar({
  pageIndex,
  totalPages,
  totalCount,
  pageSize,
  loading,
  onPrevious,
  onNext,
}: DataViewPageToolbarProps) {
  const start = totalCount === 0 ? 0 : pageIndex * pageSize + 1;
  const end = Math.min(totalCount, (pageIndex + 1) * pageSize);

  return (
    <div className="flex items-center gap-2 text-xs text-muted-foreground">
      <span>
        {totalCount === 0 ? '0 rows' : `${start}–${end} of ${totalCount}`}
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
        {pageIndex + 1} / {totalPages}
      </span>
      <Button
        type="button"
        variant="outline"
        size="sm"
        disabled={loading || pageIndex >= totalPages - 1}
        onClick={onNext}
      >
        Next
      </Button>
    </div>
  );
}
