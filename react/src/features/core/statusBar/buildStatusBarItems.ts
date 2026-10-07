import { useMemo } from "react";
import type {
  StatusBarItemRegistration,
  StatusBarItemViewModel,
  StatusBarItemsSnapshot,
  StatusBarRenderContext,
} from "./statusBarItemTypes";

function resolveClassName(
  item: StatusBarItemRegistration,
  ctx: StatusBarRenderContext,
): string | undefined {
  if (typeof item.className === "function") return item.className(ctx);
  return item.className;
}

function toViewModel(
  item: StatusBarItemRegistration,
  ctx: StatusBarRenderContext,
): StatusBarItemViewModel {
  const tooltip = item.tooltip?.(ctx);
  const ariaLabel = item.ariaLabel?.(ctx) ?? (item.onClick ? tooltip : undefined);

  return {
    id: item.id,
    alignment: item.alignment,
    priority: item.priority ?? 100,
    content: item.render(ctx),
    ariaLabel,
    tooltip,
    onClick: item.onClick ? () => item.onClick?.(ctx) : undefined,
    className: resolveClassName(item, ctx),
  };
}

export function buildStatusBarItems(
  ctx: StatusBarRenderContext,
  builtIn: StatusBarItemRegistration[],
): StatusBarItemsSnapshot {
  const all = builtIn
    .filter((item) => item.visible?.(ctx) ?? true)
    .map((item) => toViewModel(item, ctx));

  const sortByPriority = (a: StatusBarItemViewModel, b: StatusBarItemViewModel) =>
    a.priority - b.priority;

  return {
    left: all.filter((item) => item.alignment === "left").sort(sortByPriority),
    right: all.filter((item) => item.alignment === "right").sort(sortByPriority),
  };
}

export function useStatusBarSnapshot(
  ctx: StatusBarRenderContext,
  builtIn: StatusBarItemRegistration[],
): StatusBarItemsSnapshot {
  return useMemo(() => buildStatusBarItems(ctx, builtIn), [ctx, builtIn]);
}
