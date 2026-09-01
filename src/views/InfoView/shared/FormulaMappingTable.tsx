import type { ReactNode } from "react";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";

export type FormulaMappingRow = {
  symbol: string;
  variable: string;
  category?: string | null;
  coef?: number;
};

function formatCoef(
  coef: number | undefined,
  formatNum: (v: number, d?: number) => string,
): string {
  if (coef == null || Number.isNaN(coef)) return "—";
  return formatNum(coef);
}

export function FormulaMappingTable({
  mappings,
  hasCat,
  showCoef = true,
  coefHeader = "Coefficient",
  renderSymbol,
  formatNum,
}: {
  mappings: FormulaMappingRow[];
  hasCat: boolean;
  showCoef?: boolean;
  coefHeader?: string;
  renderSymbol: (symbol: string) => ReactNode;
  formatNum: (value: number, decimals?: number) => string;
}) {
  return (
    <Table className="w-full text-xs">
      <TableHeader>
        <TableRow className="border-0 hover:bg-transparent">
          <TableHead className="h-auto w-20 px-3 py-1.5 text-left font-medium text-muted-foreground">
            Symbol
          </TableHead>
          <TableHead className="h-auto px-3 py-1.5 text-left font-medium text-muted-foreground">
            Variable
          </TableHead>
          {hasCat && (
            <TableHead className="h-auto px-3 py-1.5 text-left font-medium text-muted-foreground">
              Category
            </TableHead>
          )}
          {showCoef && (
            <TableHead className="h-auto w-28 px-3 py-1.5 text-right font-medium text-muted-foreground">
              {coefHeader}
            </TableHead>
          )}
        </TableRow>
      </TableHeader>
      <TableBody>
        {mappings.map((m, idx) => (
          <TableRow
            key={`${m.symbol}-${m.variable}-${idx}`}
            className={`border-t border-border ${idx % 2 === 0 ? "bg-muted/50" : ""}`}
          >
            <TableCell className="px-3 py-1.5">{renderSymbol(m.symbol)}</TableCell>
            <TableCell className="px-3 py-1.5 font-mono text-foreground">{m.variable}</TableCell>
            {hasCat && (
              <TableCell className="px-3 py-1.5">
                {m.category != null ? (
                  <span className="inline-flex items-center rounded border border-indigo-500/25 bg-indigo-500/15 px-2 py-0.5 text-[11px] font-mono text-indigo-700 dark:text-indigo-300">
                    {m.category}
                  </span>
                ) : (
                  <span className="text-muted-foreground">—</span>
                )}
              </TableCell>
            )}
            {showCoef && (
              <TableCell className="px-3 py-1.5 text-right font-mono text-muted-foreground">
                {formatCoef(m.coef, formatNum)}
              </TableCell>
            )}
          </TableRow>
        ))}
      </TableBody>
    </Table>
  );
}
