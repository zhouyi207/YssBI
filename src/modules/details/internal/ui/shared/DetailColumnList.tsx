import { ScrollArea } from "@/components/ui/scroll-area";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import {
  detailAccentMonoTextClass,
  detailEmptyHintClass,
  detailListItemClass,
  detailNestedScrollClass,
  detailNestedTableClass,
  detailNestedTableHeadClass,
} from "./detailStyles";
import { DetailText } from "./DetailText";

interface DetailColumnListProps {
  columns: readonly { name: string; type: string }[];
  emptyMessage?: string;
  variant?: "list" | "table";
  columnLabel?: string;
  typeLabel?: string;
}

export function DetailColumnList({
  columns,
  emptyMessage,
  variant = "list",
  columnLabel,
  typeLabel,
}: DetailColumnListProps) {
  if (columns.length === 0) {
    return emptyMessage ? (
      <DetailText as="div" tone="muted" className={detailEmptyHintClass}>
        {emptyMessage}
      </DetailText>
    ) : null;
  }

  if (variant === "table") {
    return (
      <ScrollArea className={`min-w-0 max-w-full ${detailNestedScrollClass}`} orientation="both">
        <Table className={`w-max min-w-full whitespace-nowrap ${detailNestedTableClass}`}>
          {(columnLabel || typeLabel) && (
            <TableHeader>
              <TableRow className="text-muted-foreground">
                <TableHead className={detailNestedTableHeadClass}>{columnLabel}</TableHead>
                <TableHead className={detailNestedTableHeadClass}>{typeLabel}</TableHead>
              </TableRow>
            </TableHeader>
          )}
          <TableBody>
            {columns.map((column) => (
              <TableRow key={column.name} className="h-7 border-border/50">
                <TableCell className="px-3 py-1 font-medium text-foreground">
                  {column.name}
                </TableCell>
                <TableCell className={`px-3 py-1 ${detailAccentMonoTextClass}`}>
                  {column.type}
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </ScrollArea>
    );
  }

  return (
    <div className="space-y-0.5">
      {columns.map((column) => (
        <div key={column.name} className={detailListItemClass}>
          <span className="truncate">{column.name}</span>
          <span className={`ml-2 shrink-0 ${detailAccentMonoTextClass}`}>{column.type}</span>
        </div>
      ))}
    </div>
  );
}
