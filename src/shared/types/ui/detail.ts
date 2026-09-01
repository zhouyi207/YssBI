import type { DiagnosticRecordDto } from "@/shared/types/domain/diagnostics";

export type DetailTarget =
  | { kind: "node"; id: string; graphPath: string }
  | { kind: "nodeDefinition"; nodeType: string }
  | { kind: "variable"; id: string }
  | { kind: "data"; id: string }
  | { kind: "log" }
  | { kind: "event"; path: string }
  | { kind: "function"; path: string }
  | { kind: "chart"; chartPath: string };

/** Explicit user selection for the Detail panel — no derived priority chain. */
export type DetailFocus = DetailTarget;

export interface DetailTargetInput {
  detailFocus: DetailFocus | null;
  selectedLog: DiagnosticRecordDto | null;
}
