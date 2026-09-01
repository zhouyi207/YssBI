import type { CanvasInteractionScope } from "@/features/core/graphInteraction/graphInteractionStore";

let pointerScope: CanvasInteractionScope | null = null;

export function registerCanvasPointerScope(scope: CanvasInteractionScope): void {
  pointerScope = scope;
}

export function getCanvasPointerScope(): CanvasInteractionScope | null {
  return pointerScope;
}

export function clearCanvasPointerScope(graphPath?: string): void {
  if (!graphPath || pointerScope?.graphPath === graphPath) pointerScope = null;
}
