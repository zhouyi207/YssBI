import type { ReactNode } from "react";
import { Label } from "@/components/ui/label";
import { cn } from "@/lib/utils";
import { detailLabelCellClass } from "./detailStyles";

interface DetailFieldRowProps {
  label: ReactNode;
  children: ReactNode;
  labelClassName?: string;
  valueClassName?: string;
  htmlFor?: string;
  layout?: "inline" | "stacked";
}

export function DetailFieldRow({
  label,
  children,
  labelClassName,
  valueClassName,
  htmlFor,
  layout = "inline",
}: DetailFieldRowProps) {
  return (
    <div
      className={cn(
        "grid min-h-7 gap-x-2",
        layout === "stacked"
          ? "grid-cols-1 items-start gap-y-2"
          : "grid-cols-[minmax(0,2fr)_minmax(0,3fr)] items-center gap-y-1",
      )}
    >
      <Label
        htmlFor={htmlFor}
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
