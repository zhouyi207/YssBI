import type { ComponentProps } from "react";
import { Button } from "@/components/ui/button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";

export function ToolbarIconButton({
  tooltip,
  side = "bottom",
  children,
  ...props
}: ComponentProps<typeof Button> & {
  tooltip: string;
  side?: "top" | "bottom" | "left" | "right";
}) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button {...props}>{children}</Button>
      </TooltipTrigger>
      <TooltipContent side={side}>{tooltip}</TooltipContent>
    </Tooltip>
  );
}
