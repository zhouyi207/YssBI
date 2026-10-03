import { type ReactNode } from "react";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import { cn } from "@/lib/utils";
import { DetailText } from "./DetailText";

interface DetailCollapsibleSectionProps {
  title: ReactNode;
  children: ReactNode;
  defaultOpen?: boolean;
  contentClassName?: string;
}

export function DetailCollapsibleSection({
  title,
  children,
  defaultOpen = false,
  contentClassName,
}: DetailCollapsibleSectionProps) {
  return (
    <Collapsible defaultOpen={defaultOpen} className="min-w-0">
      <CollapsibleTrigger
        type="button"
        className="group/detail-section flex h-7 w-full min-w-0 cursor-default items-center gap-1.5 bg-muted/60 px-2 text-left text-foreground transition-colors select-none hover:bg-muted focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-inset focus-visible:ring-ring motion-reduce:transition-none"
      >
        <svg
          className="size-3 shrink-0 text-muted-foreground transition-transform duration-150 group-data-[state=open]/detail-section:rotate-90 motion-reduce:transition-none"
          viewBox="0 0 12 12"
          fill="currentColor"
          aria-hidden="true"
        >
          <path d="M3.5 2 8.5 6 3.5 10Z" />
        </svg>
        <DetailText
          title={typeof title === "string" ? title : undefined}
          className="min-w-0 flex-1 truncate text-xs font-semibold text-foreground"
        >
          {title}
        </DetailText>
      </CollapsibleTrigger>
      <CollapsibleContent className={cn("min-w-0 bg-background px-2 py-1.5", contentClassName)}>
        {children}
      </CollapsibleContent>
    </Collapsible>
  );
}
