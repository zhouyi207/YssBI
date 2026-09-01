import type { ReactNode } from "react";
import { Label } from "@/components/ui/label";
import { cn } from "@/lib/utils";
import { detailLabelCellClass } from "./detailStyles";

interface DetailFieldRowProps {
  label: ReactNode;
  children: ReactNode;
  labelClassName?: string;
  valueClassName?: string;
  rowClassName?: string;
}

export function DetailFieldRow({
  label,
  children,
  labelClassName,
  valueClassName,
  rowClassName,
}: DetailFieldRowProps) {
  return (
    <div
      className={cn(
        "grid min-h-10 grid-cols-[minmax(0,2fr)_minmax(0,3fr)] items-center gap-2",
        rowClassName,
      )}
    >
      <Label
        title={typeof label === "string" ? label : undefined}
        className={cn(
          detailLabelCellClass,
          "min-w-0 w-full truncate justify-start",
          labelClassName,
        )}
      >
        {label}
      </Label>
      <div className={cn("min-w-0 w-full text-right", valueClassName)}>{children}</div>
    </div>
  );
}
