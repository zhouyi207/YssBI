import { Pin } from "@/shared/types/editor";
import { canConnect } from "@/features/shema/shema.helpers";

export const isSingleLinkPin = (p: Pin) => p.type === "exec" || p.direction === "input";

/**
 * 检查两个针脚是否兼容
 * 优先使用 schema store 的类型兼容性规则
 */
export const isCompatiblePins = (a: Pin, b: Pin) => {
    // 方向必须不同（一个输入，一个输出）
    if (a.direction === b.direction) return false;
    
    // 确定哪个是输出（源）哪个是输入（目标）
    const [source, target] = a.direction === "output" ? [a, b] : [b, a];
    
    // 使用 schema helper 检查兼容性
    if (canConnect(source.type, target.type)) {
        return true;
    }
    
    // 后备逻辑：基本类型检查
    if (source.type === target.type) return true;
    if (source.type === "exec" || target.type === "exec") return false;
    if (target.type === "object") return true; // object 可以接受任何非 exec 类型
    if ((source.type === "int" && target.type === "float") || 
        (source.type === "float" && target.type === "int")) return true;

    return false;
};
