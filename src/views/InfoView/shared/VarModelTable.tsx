import type { ReactNode } from 'react';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table';
import { cn } from '@/lib/utils';
import { formatNum } from './RegressionShared';

export const infoVarHeadClass =
  'h-auto px-4 py-2.5 text-[11px] font-medium uppercase tracking-wider text-muted-foreground';

export const infoVarCellClass = 'px-4 py-2.5 font-mono text-foreground';

export function VarModelTable({
  columns,
  children,
  className,
  tableClassName,
  footer,
}: {
  columns: ReactNode[];
  children: ReactNode;
  className?: string;
  tableClassName?: string;
  footer?: ReactNode;
}) {
  return (
    <div className={cn('overflow-hidden rounded-lg border border-border bg-muted', className)}>
      <Table className={cn('w-full text-left text-sm', tableClassName)}>
        <TableHeader>
          <TableRow className="border-b border-border hover:bg-transparent">
            {columns.map((col, index) => (
              <TableHead key={index} className={infoVarHeadClass}>
                {col}
              </TableHead>
            ))}
          </TableRow>
        </TableHeader>
        <TableBody>{children}</TableBody>
      </Table>
      {footer}
    </div>
  );
}

export function VarModelRow({
  children,
  className,
}: {
  children: ReactNode;
  className?: string;
}) {
  return (
    <TableRow className={cn('border-b border-border last:border-b-0 hover:bg-muted/40', className)}>
      {children}
    </TableRow>
  );
}

export function VarModelCell({
  children,
  className,
}: {
  children: ReactNode;
  className?: string;
}) {
  return <TableCell className={cn(infoVarCellClass, className)}>{children}</TableCell>;
}

export function VarEigenvalueTable({
  rows,
}: {
  rows: { re: number; im: number; modulus: number }[];
}) {
  return (
    <Table className="min-w-[200px] text-left text-sm">
      <TableHeader>
        <TableRow className="border-b border-border hover:bg-transparent">
          <TableHead className={infoVarHeadClass}>Eigenvalue</TableHead>
          <TableHead className={infoVarHeadClass}>Modulus</TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        {rows.map((row, index) => (
          <VarModelRow key={index}>
            <VarModelCell>
              {row.im >= 0
                ? `${formatNum(row.re)} + ${formatNum(row.im)}i`
                : `${formatNum(row.re)} - ${formatNum(Math.abs(row.im))}i`}
            </VarModelCell>
            <VarModelCell>{formatNum(row.modulus)}</VarModelCell>
          </VarModelRow>
        ))}
      </TableBody>
    </Table>
  );
}
