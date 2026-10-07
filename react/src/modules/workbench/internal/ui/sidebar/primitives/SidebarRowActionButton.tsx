import { VscChevronRight } from "react-icons/vsc";
import { Button } from "@/components/ui/button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { sidebarRowActionClass, SIDEBAR_ROW_ICON_SIZE } from "./sidebarStyles";

export function SidebarRowActionButton({
  isSelected = false,
  tooltip,
  onClick,
  icon,
}: {
  isSelected?: boolean;
  tooltip: string;
  onClick: (e: React.MouseEvent) => void;
  icon?: React.ReactNode;
}) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button
          type="button"
          variant="ghost"
          size="icon-xs"
          onClick={onClick}
          className={sidebarRowActionClass(isSelected)}
        >
          {icon ?? <VscChevronRight size={SIDEBAR_ROW_ICON_SIZE} />}
        </Button>
      </TooltipTrigger>
      <TooltipContent side="top">{tooltip}</TooltipContent>
    </Tooltip>
  );
}
