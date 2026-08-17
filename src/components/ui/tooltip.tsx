"use client"

import * as React from "react"
import { Tooltip as TooltipPrimitive } from "radix-ui"

import { cn } from "@/lib/utils"
import { addGlobalEventListener } from "@/shared/utils/globalEvent"

type ActiveTooltip = {
  close: () => void
}

type TooltipCoordinator = {
  registerActiveTooltip: (close: () => void) => () => void
  requestOpenChange: (change: () => void) => void
}

const TooltipCoordinatorContext = React.createContext<TooltipCoordinator>({
  registerActiveTooltip: () => () => undefined,
  requestOpenChange: (change) => change(),
})

function TooltipProvider({
  delayDuration = 0,
  children,
  ...props
}: React.ComponentProps<typeof TooltipPrimitive.Provider>) {
  const windowDraggingRef = React.useRef(false)
  const activeTooltipRef = React.useRef<ActiveTooltip | null>(null)
  const coordinator = React.useMemo<TooltipCoordinator>(() => ({
    registerActiveTooltip: (close) => {
      const activeTooltip = { close }
      activeTooltipRef.current = activeTooltip

      return () => {
        if (activeTooltipRef.current === activeTooltip) {
          activeTooltipRef.current = null
        }
      }
    },
    requestOpenChange: (change) => {
      if (!windowDraggingRef.current) change()
    },
  }), [])

  React.useEffect(() => {
    const dragStart = () => {
      if (windowDraggingRef.current) return

      windowDraggingRef.current = true
      activeTooltipRef.current?.close()
    }
    const dragEnd = () => {
      if (!windowDraggingRef.current) return

      windowDraggingRef.current = false
    }
    const cleanupDragStart = addGlobalEventListener(window, "yssbi-window-drag-start", dragStart)
    const cleanupDragEnd = addGlobalEventListener(window, "yssbi-window-drag-end", dragEnd)

    return () => {
      cleanupDragStart()
      cleanupDragEnd()
    }
  }, [])

  return (
    <TooltipCoordinatorContext.Provider value={coordinator}>
      <TooltipPrimitive.Provider
        data-slot="tooltip-provider"
        delayDuration={delayDuration}
        {...props}
      >
        {children}
      </TooltipPrimitive.Provider>
    </TooltipCoordinatorContext.Provider>
  )
}

function Tooltip({
  open: controlledOpen,
  defaultOpen = false,
  onOpenChange,
  ...props
}: React.ComponentProps<typeof TooltipPrimitive.Root>) {
  const { registerActiveTooltip, requestOpenChange } = React.useContext(TooltipCoordinatorContext)
  const [uncontrolledOpen, setUncontrolledOpen] = React.useState(defaultOpen)
  const controlled = controlledOpen !== undefined
  const open = controlled ? controlledOpen : uncontrolledOpen
  const dragCloseRef = React.useRef<() => void>(() => undefined)

  dragCloseRef.current = () => {
    if (!open) return
    if (!controlled) setUncontrolledOpen(false)
    onOpenChange?.(false)
  }

  const closeForWindowDrag = React.useCallback(() => dragCloseRef.current(), [])

  React.useEffect(() => {
    if (!open) return
    return registerActiveTooltip(closeForWindowDrag)
  }, [closeForWindowDrag, open, registerActiveTooltip])

  return (
    <TooltipPrimitive.Root
      data-slot="tooltip"
      {...props}
      open={open}
      onOpenChange={(nextOpen) => {
        requestOpenChange(() => {
          if (!controlled) setUncontrolledOpen(nextOpen)
          onOpenChange?.(nextOpen)
        })
      }}
    />
  )
}

function TooltipTrigger({
  ...props
}: React.ComponentProps<typeof TooltipPrimitive.Trigger>) {
  return <TooltipPrimitive.Trigger data-slot="tooltip-trigger" {...props} />
}

function TooltipContent({
  className,
  sideOffset = 0,
  children,
  ...props
}: React.ComponentProps<typeof TooltipPrimitive.Content>) {
  return (
    <TooltipPrimitive.Portal>
      <TooltipPrimitive.Content
        data-slot="tooltip-content"
        sideOffset={sideOffset}
        className={cn(
          "z-50 inline-flex w-fit max-w-xs origin-(--radix-tooltip-content-transform-origin) items-center gap-1.5 rounded-md bg-foreground px-3 py-1.5 text-xs text-background has-data-[slot=kbd]:pr-1.5 data-[side=bottom]:slide-in-from-top-2 data-[side=left]:slide-in-from-right-2 data-[side=right]:slide-in-from-left-2 data-[side=top]:slide-in-from-bottom-2 **:data-[slot=kbd]:relative **:data-[slot=kbd]:isolate **:data-[slot=kbd]:z-50 **:data-[slot=kbd]:rounded-sm data-[state=delayed-open]:animate-in data-[state=delayed-open]:fade-in-0 data-[state=delayed-open]:zoom-in-95 data-open:animate-in data-open:fade-in-0 data-open:zoom-in-95 data-closed:animate-out data-closed:fade-out-0 data-closed:zoom-out-95",
          className
        )}
        {...props}
      >
        {children}
        <TooltipPrimitive.Arrow className="z-50 size-2.5 translate-y-[calc(-50%_-_2px)] rotate-45 rounded-[2px] bg-foreground fill-foreground" />
      </TooltipPrimitive.Content>
    </TooltipPrimitive.Portal>
  )
}

export { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger }
