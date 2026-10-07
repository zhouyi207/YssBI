import type { ApplyGraphMutationOutcome } from "@/features/application/graphEditing/graphEditCoordinator";

export type GraphEditOutcome =
  | ApplyGraphMutationOutcome
  | { status: "unavailable" }
  | { status: "failed" };

export interface CommandHandler<TArgs = unknown, TResult = unknown> {
  execute(graphPath: string, args: TArgs): Promise<TResult> | TResult;
}
