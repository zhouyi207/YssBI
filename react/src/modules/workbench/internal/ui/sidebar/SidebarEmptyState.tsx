import type { ReactNode } from "react";
import {
  Empty,
  EmptyContent,
  EmptyDescription,
  EmptyHeader,
  EmptyTitle,
} from "@/components/ui/empty";
import { cn } from "@/lib/utils";

export function SidebarEmptyState({
  title,
  description,
  action,
  className,
}: {
  title: string;
  description?: string;
  action?: ReactNode;
  className?: string;
}) {
  return (
    <Empty className={cn("min-w-0 gap-2 rounded-none px-3 py-4 text-xs", className)}>
      <EmptyHeader className="items-start text-left">
        <EmptyTitle className="break-words text-xs font-normal text-foreground/85">
          {title}
        </EmptyTitle>
        {description ? (
          <EmptyDescription className="break-words text-left leading-relaxed">
            {description}
          </EmptyDescription>
        ) : null}
      </EmptyHeader>
      {action ? <EmptyContent className="items-start">{action}</EmptyContent> : null}
    </Empty>
  );
}
