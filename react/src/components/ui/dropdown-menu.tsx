import type { ComponentProps } from "react";
import { DropdownMenu as Primitive } from "radix-ui";
import { cn } from "@/lib/utils";
export const DropdownMenu = Primitive.Root;
export const DropdownMenuTrigger = Primitive.Trigger;
export function DropdownMenuContent({
  className,
  sideOffset = 4,
  ...props
}: ComponentProps<typeof Primitive.Content>) {
  return (
    <Primitive.Portal>
      <Primitive.Content
        sideOffset={sideOffset}
        className={cn(
          "z-[502] min-w-36 rounded-md border border-border bg-popover p-1 text-popover-foreground shadow-md",
          className,
        )}
        {...props}
      />
    </Primitive.Portal>
  );
}
export function DropdownMenuItem({ className, ...props }: ComponentProps<typeof Primitive.Item>) {
  return (
    <Primitive.Item
      className={cn(
        "flex cursor-default items-center rounded-sm px-2 py-1.5 text-xs outline-none focus:bg-accent focus:text-accent-foreground data-[disabled]:pointer-events-none data-[disabled]:opacity-50",
        className,
      )}
      {...props}
    />
  );
}
