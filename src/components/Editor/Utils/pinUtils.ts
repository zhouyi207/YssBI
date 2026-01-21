import { Pin } from "../Types/nodes";

export const isSingleLinkPin = (p: Pin) => p.type === "exec" || p.direction === "input";

export const isCompatiblePins = (a: Pin, b: Pin) => {
    if (a.direction === b.direction) return false;
    if (a.type === b.type) return true;

    if (a.type === "exec" || b.type === "exec") return false;
    if (a.type === "object" || b.type === "object") return true;

    if ((a.type === "int" && b.type === "float") || (a.type === "float" && b.type === "int")) return true;

    return false;
};
