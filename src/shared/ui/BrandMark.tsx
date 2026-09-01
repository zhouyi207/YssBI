import type { ComponentProps } from "react";
import { cn } from "@/lib/utils";

export function BrandMark({ className, ...props }: ComponentProps<"span">) {
  return (
    <span
      aria-hidden="true"
      className={cn(
        "relative inline-flex size-6 shrink-0 items-center justify-center overflow-hidden rounded-md",
        "bg-[var(--accent-color)] text-white shadow-[0_0_0_1px_color-mix(in_srgb,var(--accent-color)_55%,transparent),0_5px_16px_color-mix(in_srgb,var(--accent-color)_24%,transparent)]",
        className,
      )}
      {...props}
    >
      <svg viewBox="0 0 24 24" fill="none" className="size-[72%]" focusable="false">
        <path
          d="M5 5.5h2.2c2.65 0 4.8 2.15 4.8 4.8v3.4c0 2.65 2.15 4.8 4.8 4.8H19"
          stroke="currentColor"
          strokeWidth="1.8"
          strokeLinecap="round"
        />
        <path
          d="M19 5.5h-2.2A4.8 4.8 0 0 0 12 10.3"
          stroke="currentColor"
          strokeWidth="1.8"
          strokeLinecap="round"
        />
        <circle cx="5" cy="5.5" r="1.55" fill="currentColor" />
        <circle cx="19" cy="5.5" r="1.55" fill="currentColor" />
        <circle cx="19" cy="18.5" r="1.55" fill="currentColor" />
      </svg>
    </span>
  );
}

export function BrandLockup({ className, ...props }: ComponentProps<"div">) {
  return (
    <div className={cn("flex items-center gap-2.5", className)} {...props}>
      <BrandMark />
      <span className="font-heading text-[13px] font-semibold tracking-[-0.025em] text-foreground">
        Yss<span className="text-[var(--accent-color)]">BI</span>
      </span>
    </div>
  );
}
