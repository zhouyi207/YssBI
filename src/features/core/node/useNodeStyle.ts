import { useSchemaStore } from '@/features/core/schema';
import { Node } from '@/shared/types/ui';

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
export function useNodeStyle(node: Node) {
  // 优先从 schema 获取 centerSymbol，回退到节点属性
  const schemaCenterSymbol = useSchemaStore((_s) => undefined);
  
  const centerSymbol = schemaCenterSymbol ?? node.centerSymbol;

  return {
    centerSymbol,
    uiStyle: node.ui_style,
  };
}
