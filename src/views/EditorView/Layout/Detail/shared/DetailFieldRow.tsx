import type { ReactNode } from 'react';
import { Label } from '@/components/ui/label';
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
    <div className={cn('grid min-h-10 grid-cols-[auto_minmax(0,1fr)] items-center gap-3', rowClassName)}>
      <Label className={cn(labelWidthClass, 'shrink-0 justify-start', labelClassName)}>
        {label}
      </Label>
      <div className={cn('min-w-0', valueClassName)}>{children}</div>
    </div>
  );
}
