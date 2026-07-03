/**
 * Node Class Names Utility
 *
 * 职责：
 * - 根据节点状态生成 CSS 类名
 * - 集中管理节点样式逻辑
 *
 * 执行中样式由 [data-exec-state] + App.css 承担（live/replay 阶段）。
 */

interface NodeClassNameOptions {
  selected?: boolean;
  hasError?: boolean;
  isCompleted?: boolean;
}

/**
 * 获取节点容器的 className
 */
export function getNodeClassName({
  selected,
  hasError,
  isCompleted,
}: NodeClassNameOptions): string {
  const baseClasses = "absolute select-none rounded border cursor-move shadow-[var(--node-shadow)]";

  if (selected) {
    return `${baseClasses} border-[var(--accent-color)] ring-2 ring-[var(--accent-color)]/50 z-30`;
  }

  if (hasError) {
    return `${baseClasses} border-red-500 ring-2 ring-red-500/50 z-30`;
  }

  if (isCompleted) {
    return `${baseClasses} border-green-500 ring-1 ring-green-500/30 z-20`;
  }

  return `${baseClasses} border-[var(--node-border)] z-10`;
}

/**
 * 获取节点背景样式
 */
export function getNodeBackgroundStyle({
  hasError,
  isCompleted,
}: Pick<NodeClassNameOptions, 'hasError' | 'isCompleted'>): string {
  if (hasError) {
    return "linear-gradient(135deg, var(--node-base) 0%, rgba(239, 68, 68, 0.1) 100%)";
  }

  if (isCompleted) {
    return "linear-gradient(135deg, var(--node-base) 0%, rgba(34, 197, 94, 0.12) 100%)";
  }

  return "var(--node-base)";
}

/**
 * 获取节点最小尺寸
 */
export function getNodeMinSize(noHeader?: boolean) {
  return {
    minWidth: noHeader ? 120 : 160,
    minHeight: noHeader ? 60 : undefined,
  };
}
