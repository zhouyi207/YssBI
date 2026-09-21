import type { IJsonModel } from "flexlayout-react";

export function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/** Check identity and bounded tree structure before native model normalization can hide errors. */
export function isLayoutJson(
  value: unknown,
  validateTab: (tab: Record<string, unknown>, parent: string, floating: boolean) => boolean,
  allowFloats = false,
): value is IJsonModel {
  if (!isRecord(value) || !isRecord(value.layout) || value.layout.type !== "row") return false;
  if (value.global !== undefined && !isRecord(value.global)) return false;
  if (
    value.subLayouts !== undefined &&
    (!isRecord(value.subLayouts) || (!allowFloats && Object.keys(value.subLayouts).length))
  )
    return false;
  if (
    value.popouts !== undefined &&
    (!isRecord(value.popouts) || Object.keys(value.popouts).length)
  )
    return false;
  const ids = new Set<string>();
  let count = 0;
  const visit = (node: unknown, parent: string, depth: number, floating = false): boolean => {
    if (++count > 1024 || depth > 24 || !isRecord(node)) return false;
    if (node.id !== undefined) {
      if (typeof node.id !== "string" || !node.id || ids.has(node.id)) return false;
      ids.add(node.id);
    }
    for (const field of ["weight", "size", "minWidth", "minHeight", "minSize"]) {
      if (
        node[field] !== undefined &&
        (typeof node[field] !== "number" || !Number.isFinite(node[field]) || node[field] < 0)
      )
        return false;
    }
    if (node.type === "tab")
      return (
        typeof node.id === "string" &&
        node.subLayoutId === undefined &&
        validateTab(node, parent, floating)
      );
    if (!["row", "tabset", "border"].includes(String(node.type))) return false;
    if (node.children !== undefined && !Array.isArray(node.children)) return false;
    const children = (node.children ?? []) as unknown[];
    if (
      node.selected !== undefined &&
      (typeof node.selected !== "number" ||
        !Number.isInteger(node.selected) ||
        node.selected < -1 ||
        node.selected >= Math.max(1, children.length))
    )
      return false;
    return children.every(
      (child) =>
        isRecord(child) &&
        (node.type === "row"
          ? child.type === "row" || child.type === "tabset"
          : child.type === "tab") &&
        visit(
          child,
          node.type === "border" ? "border_" + node.location : String(node.id ?? ""),
          depth + 1,
          floating,
        ),
    );
  };
  if (!visit(value.layout, "", 0)) return false;
  for (const [layoutId, layout] of Object.entries(value.subLayouts ?? {})) {
    if (
      !layoutId ||
      layoutId === "__main_layout_id__" ||
      ids.has(layoutId) ||
      !isRecord(layout) ||
      layout.type !== "float" ||
      !isRecord(layout.layout) ||
      layout.layout.type !== "row" ||
      !isRecord(layout.rect)
    )
      return false;
    ids.add(layoutId);
    const rect = layout.rect;
    if (
      !["x", "y", "width", "height"].every(
        (key) => typeof rect[key] === "number" && Number.isFinite(rect[key]),
      ) ||
      (rect.width as number) <= 0 ||
      (rect.height as number) <= 0
    )
      return false;
    if (!visit(layout.layout, "", 0, true)) return false;
  }
  if (value.borders !== undefined && !Array.isArray(value.borders)) return false;
  const positions = new Set<string>();
  for (const border of (value.borders ?? []) as unknown[]) {
    if (
      !isRecord(border) ||
      border.type !== "border" ||
      !["left", "right", "bottom", "top"].includes(String(border.location)) ||
      positions.has(String(border.location))
    )
      return false;
    positions.add(String(border.location));
    if (!visit(border, "", 0)) return false;
  }
  return true;
}
