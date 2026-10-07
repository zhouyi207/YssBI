import { LoadStatus } from "@/shared/types/ui";

export interface InitializationState {
  status: LoadStatus;
  error: string | null;
}
