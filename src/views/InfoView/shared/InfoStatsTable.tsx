import type { ReactNode } from "react";
import { Table } from "@/components/ui/table";
import { cn } from "@/lib/utils";

export function InfoStatsTable({
  children,
  className,
  tableClassName,
}: {
  children: ReactNode;
  className?: string;
  tableClassName?: string;
}) {
  return (
    <div className={cn("overflow-hidden rounded-lg border border-border", className)}>
      <Table className={cn("w-full text-xs", tableClassName)}>{children}</Table>
    </div>
  );
}

export const infoStatsHeadClass =
  "h-auto bg-muted px-4 py-2.5 text-left text-muted-foreground font-medium uppercase tracking-wider";

export const infoStatsHeadCompactClass =
  "h-auto bg-muted px-3 py-2.5 text-right text-muted-foreground font-medium uppercase tracking-wider";

export const infoStatsRowEvenClass = "border-t border-border bg-card hover:bg-muted";

export const infoStatsRowOddClass = "border-t border-border bg-muted/40 hover:bg-muted";

export const infoStatsCellClass = "px-4 py-2.5";

export const infoStatsCellCompactClass = "px-3 py-2.5";

export const infoStatsCellRightClass = "px-3 py-2.5 text-right font-mono";
