import type { MindSnapshot } from "@/shared/types/domain/mind";
import { createFileProjectionStore } from "./fileProjectionStore";
export const mindProjection = createFileProjectionStore<MindSnapshot>();
export const useMindProjectionStore = mindProjection.store;
