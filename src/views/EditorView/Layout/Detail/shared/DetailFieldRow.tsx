import type { ReactNode } from 'react';
import { TableCell, TableRow } from '@/components/ui/table';
import { cn } from '@/lib/utils';
import {
  detailLabelCellClass,
  detailLabelCellNarrowClass,
  detailLabelCellWideClass,
} from './detailStyles';

interface DetailFieldRowProps {
  label: ReactNode;
  children: ReactNode;
  labelWidth?: 'narrow' | 'wide' | 'auto';
  labelClassName?: string;
  valueClassName?: string;
  rowClassName?: string;
}

export function DetailFieldRow({
  label,
  children,
  labelWidth = 'narrow',
  labelClassName,
  valueClassName,
  rowClassName,
}: DetailFieldRowProps) {
  const labelWidthClass =
    labelWidth === 'wide'
      ? detailLabelCellWideClass
      : labelWidth === 'auto'
        ? detailLabelCellClass
        : detailLabelCellNarrowClass;

  return (
    <TableRow className={rowClassName}>
      <TableCell className={cn(labelWidthClass, labelClassName)}>{label}</TableCell>
      <TableCell className={valueClassName}>{children}</TableCell>
    </TableRow>
  );
}
