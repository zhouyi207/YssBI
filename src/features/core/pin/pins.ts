import { Pin } from "@/shared/types/domain";
import { isExecPin } from "@/shared/types/domain/pinSemantics";

export const isSingleLinkPin = (p: Pin) => isExecPin(p) || p.direction === "input";
