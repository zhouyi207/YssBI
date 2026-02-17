import { Pin } from "@/shared/types/domain";

export const isSingleLinkPin = (p: Pin) => p.type === "exec" || p.direction === "input";
