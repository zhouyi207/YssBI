/**
 * Node Class Names Utility
 * 
 * 职责：
 * - 根据节点状态生成 CSS 类名
 * - 集中管理节点样式逻辑
 */

interface NodeClassNameOptions {
  selected?: boolean;
  isExecuting?: boolean;
  hasError?: boolean;
  isCompleted?: boolean;
}

/**
 * 获取节点容器的 className
 */
export function getNodeClassName({
  selected,
  isExecuting,
  hasError,
  isCompleted,
}: NodeClassNameOptions): string {
  const baseClasses = "absolute select-none rounded shadow-2xl border cursor-move";
  
  if (selected) {
    return `${baseClasses} border-[var(--accent-color)] ring-2 ring-[var(--accent-color)]/50 z-30`;
  }
  
  if (isExecuting) {
    return `${baseClasses} border-yellow-400 ring-2 ring-yellow-400/50 z-30 animate-pulse`;
  }
  
  if (hasError) {
    return `${baseClasses} border-red-500 ring-2 ring-red-500/50 z-30`;
  }
  
  if (isCompleted) {
    return `${baseClasses} border-green-500/50 z-20`;
  }
  
  return `${baseClasses} border-[#2b2b2b] z-10`;
}

/**
 * 获取节点背景样式
 */
export function getNodeBackgroundStyle({
  isExecuting,
  hasError,
  isCompleted,
}: Pick<NodeClassNameOptions, 'isExecuting' | 'hasError' | 'isCompleted'>): string {
  if (isExecuting) {
    return "linear-gradient(135deg, var(--node-base) 0%, rgba(250, 204, 21, 0.1) 100%)";
  }
  
  if (hasError) {
    return "linear-gradient(135deg, var(--node-base) 0%, rgba(239, 68, 68, 0.1) 100%)";
  }
  
  if (isCompleted) {
    return "linear-gradient(135deg, var(--node-base) 0%, rgba(34, 197, 94, 0.05) 100%)";
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
