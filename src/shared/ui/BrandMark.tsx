import type { ComponentProps } from "react";
import { cn } from "@/lib/utils";

export function BrandLockup({ className, ...props }: ComponentProps<"div">) {
  return (
    <div className={cn("flex items-center", className)} {...props}>
      <span className="font-heading text-[15px] font-semibold tracking-[-0.025em] text-foreground">
        Yss<span className="text-[var(--accent-color)]">BI</span>
      </span>
    </div>
  );
}
