import type { ReactNode } from "react";
import { formatNum } from "@/shared/stats/formatStat";
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
  coef?: number;
};

function formatCoef(coef: number | undefined): string {
  if (coef == null || Number.isNaN(coef)) return "—";
  return formatNum(coef);
}

export function FormulaMappingTable({
  mappings,
  renderSymbol,
}: {
  mappings: FormulaMappingRow[];
  renderSymbol: (symbol: string) => ReactNode;
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
          <TableHead className="h-auto w-28 px-3 py-1.5 text-right font-medium text-muted-foreground">
            Coefficient
          </TableHead>
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
            <TableCell className="px-3 py-1.5 text-right font-mono text-muted-foreground">
              {formatCoef(m.coef)}
            </TableCell>
          </TableRow>
        ))}
      </TableBody>
    </Table>
  );
}
