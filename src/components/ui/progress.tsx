import * as React from "react";
import { Progress as ProgressPrimitive } from "radix-ui";

import { cn } from "@/lib/utils";

export interface ProgressProps extends React.ComponentPropsWithoutRef<typeof ProgressPrimitive.Root> {
  /** 上限，与 `value` 组成 0~max 的比例；默认 1 表示 `value` 为 0~1 */
  max?: number;
}

const Progress = React.forwardRef<React.ElementRef<typeof ProgressPrimitive.Root>, ProgressProps>(
  ({ className, value, max = 1, ...props }, ref) => {
    const pct =
      typeof value === "number" && max > 0 && Number.isFinite(value)
        ? Math.min(100, Math.max(0, (value / max) * 100))
        : 0;

    return (
      <ProgressPrimitive.Root
        ref={ref}
        max={max}
        value={value}
        className={cn("relative h-2 w-full overflow-hidden rounded-full bg-muted", className)}
        {...props}
      >
        <ProgressPrimitive.Indicator
          className="h-full w-full flex-1 bg-primary transition-transform duration-150 ease-out"
          style={{ transform: `translateX(-${100 - pct}%)` }}
        />
      </ProgressPrimitive.Root>
    );
  },
);
Progress.displayName = "Progress";

export { Progress };
