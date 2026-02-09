import { LoadStatus } from "@/shared/types/loadStatus";

export interface InitializationState {
  status: LoadStatus;
  error: string | null;
}