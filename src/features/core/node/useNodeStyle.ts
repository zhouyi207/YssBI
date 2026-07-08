import type { UINode } from '@/shared/types/ui';

/**
 * Node Style Hook
 * 
 * 职责：
 * - 获取节点的样式配置
 * - 提供节点的中心符号（如数学运算符）
 * 
 * 使用场景：
 * - Math Node 需要显示中心符号
 * - 需要根据 schema 获取样式配置
 */
export function useNodeStyle(node: UINode) {
  return {
    centerSymbol: node.centerSymbol,
    uiStyle: node.uiStyle,
  };
}
