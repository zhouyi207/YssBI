import { VscChevronDown } from "react-icons/vsc";
import { cn } from "@/lib/utils";

export const SIDEBAR_CHEVRON_SIZE = 12 as const;

export function SidebarChevron({
  expanded,
  size = SIDEBAR_CHEVRON_SIZE,
  className,
}: {
  expanded: boolean;
  size?: typeof SIDEBAR_CHEVRON_SIZE;
  className?: string;
}) {
  return (
    <span
      className={cn(
        "shrink-0 text-muted-foreground transition-transform duration-150 ease-out",
        className,
      )}
      style={{ transform: expanded ? "rotate(0deg)" : "rotate(-90deg)" }}
    >
      <VscChevronDown size={size} />
    </span>
  );
}
