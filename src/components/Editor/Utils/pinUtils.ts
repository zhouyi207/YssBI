import { Pin } from "../Types/nodes";
import { useSchemaStore } from "../Store/useSchemaStore";

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
    
    // 使用 schema store 检查兼容性
    const canConnect = useSchemaStore.getState().canConnect;
    if (canConnect(source.type, target.type)) {
        return true;
    }
    
    // 后备逻辑：如果 schema 未加载，使用硬编码规则
    if (!useSchemaStore.getState().isLoaded) {
        if (source.type === target.type) return true;
        if (source.type === "exec" || target.type === "exec") return false;
        if (target.type === "object") return true; // object 可以接受任何非 exec 类型
        if ((source.type === "int" && target.type === "float") || 
            (source.type === "float" && target.type === "int")) return true;
    }

    return false;
};
