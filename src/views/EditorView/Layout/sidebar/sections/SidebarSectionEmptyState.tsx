import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { sidebarItemIndent } from "../../sidebarUi/sidebarStyles";

export function SidebarSectionEmptyState({
  level,
  message,
  onContextMenu,
}: {
  level: number;
  message: string;
  onContextMenu?: (event: React.MouseEvent) => void;
}) {
  return (
    <div
      className="flex h-7 w-full min-w-0 items-center pr-2 text-[12px] text-muted-foreground/70"
      style={sidebarItemIndent(level)}
      onContextMenu={onContextMenu}
    >
      <Tooltip>
        <TooltipTrigger asChild>
          <span className="block min-w-0 flex-1 truncate" aria-label={message} tabIndex={0}>
            {message}
          </span>
        </TooltipTrigger>
        <TooltipContent side="right">{message}</TooltipContent>
      </Tooltip>
    </div>
  );
}
