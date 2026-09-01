import type { ResultValue } from "./types";
import type { DeepReadonly } from "@/shared/types/deepReadonly";

export function resultValuePayload(source: DeepReadonly<ResultValue>): unknown {
  return source.value;
}

export function reportResultValuePayload(source: DeepReadonly<ResultValue>): unknown {
  if (source.kind !== "value") {
    throw new Error("Report results require a canonical value object");
  }
  return resultValuePayload(source);
}
