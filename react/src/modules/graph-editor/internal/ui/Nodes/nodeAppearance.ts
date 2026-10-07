export const REROUTE_NODE_WIDTH_PX = 32;
export const REROUTE_NODE_HEIGHT_PX = 20;
export const REROUTE_GRIP_SIZE_PX = 8;

interface NodeClassNameOptions {
  selected?: boolean;
  hasError?: boolean;
  isCompleted?: boolean;
}

export function getNodeClassName({
  selected,
  hasError,
  isCompleted,
}: NodeClassNameOptions): string {
  const baseClasses =
    "relative select-none rounded-md border cursor-move shadow-[var(--node-shadow)]";

  if (selected) {
    return `${baseClasses} border-[var(--accent-color)] ring-2 ring-[var(--accent-color)]/50 z-30`;
  }

  if (hasError) {
    return `${baseClasses} border-[var(--status-danger)] ring-2 ring-[var(--status-danger)]/50 z-30`;
  }

  if (isCompleted) {
    return `${baseClasses} border-[var(--status-success)] ring-1 ring-[var(--status-success)]/30 z-20`;
  }

  return `${baseClasses} border-[var(--node-border)] z-10`;
}

export function getNodeBackgroundStyle({
  hasError,
  isCompleted,
}: Pick<NodeClassNameOptions, "hasError" | "isCompleted">): string {
  if (hasError) {
    return "linear-gradient(135deg, var(--node-base) 0%, color-mix(in srgb, var(--status-danger) 10%, transparent) 100%)";
  }

  if (isCompleted) {
    return "linear-gradient(135deg, var(--node-base) 0%, color-mix(in srgb, var(--status-success) 12%, transparent) 100%)";
  }

  return "var(--node-base)";
}

export function getNodeMinSize(compactReroute = false) {
  if (compactReroute) {
    return {
      width: REROUTE_NODE_WIDTH_PX,
      height: REROUTE_NODE_HEIGHT_PX,
      minWidth: REROUTE_NODE_WIDTH_PX,
      minHeight: REROUTE_NODE_HEIGHT_PX,
    };
  }
  return {
    minWidth: 160,
  };
}
