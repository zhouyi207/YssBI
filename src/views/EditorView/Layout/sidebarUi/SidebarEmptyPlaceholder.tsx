import { cn } from "@/lib/utils";

export function SidebarEmptyPlaceholder({
  children,
  className,
}: {
  children: React.ReactNode;
  className?: string;
}) {
  return (
    <div className={cn("py-1.5 pl-4 text-[12px] text-muted-foreground/70", className)}>
      {children}
    </div>
  );
}
