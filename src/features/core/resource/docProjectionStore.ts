import type { DocSnapshot } from "@/shared/types/domain/doc";
import { createFileProjectionStore } from "./fileProjectionStore";
export const docProjection = createFileProjectionStore<DocSnapshot>();
export const useDocProjectionStore = docProjection.store;
