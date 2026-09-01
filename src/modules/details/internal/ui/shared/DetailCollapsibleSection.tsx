import { type ReactNode } from "react";
import { Button } from "@/components/ui/button";
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
    <Collapsible defaultOpen={defaultOpen} className="group/collapsible border-b border-border/60">
      <CollapsibleTrigger asChild>
        <Button
          type="button"
          variant="ghost"
          className="h-7 w-full justify-between gap-3 rounded-none px-3 py-0 text-left hover:bg-transparent aria-expanded:bg-transparent aria-expanded:text-inherit dark:aria-expanded:bg-transparent"
        >
          <DetailText className="min-w-0 truncate text-xs font-semibold text-foreground">
            {title}
          </DetailText>
          <svg
            className="size-3.5 shrink-0 text-muted-foreground transition-transform group-data-[state=open]/collapsible:rotate-180"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            aria-hidden="true"
          >
            <path d="m6 9 6 6 6-6" />
          </svg>
        </Button>
      </CollapsibleTrigger>
      <CollapsibleContent className={cn("px-3 pb-2 pt-1", contentClassName)}>
        {children}
      </CollapsibleContent>
    </Collapsible>
  );
}
