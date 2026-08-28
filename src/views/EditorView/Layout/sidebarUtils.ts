import { PIN_COLORS } from "@/features/application/viewCapabilities";
import type { DataType } from "@/shared/types/domain/dataType";
import { dataTypeDisplay } from "@/shared/types/domain/dataType";

export function safeDataTypeDisplay(dataType: unknown): string {
  if (typeof dataType === "string") return dataType;
  if (dataType && typeof dataType === "object" && "kind" in dataType) {
    return dataTypeDisplay(dataType as DataType);
  }
  return "";
}

export function safeDataTypeColor(dataType: unknown): string {
  if (typeof dataType === "string") return PIN_COLORS[dataType] ?? "var(--muted-foreground)";
  if (dataType && typeof dataType === "object" && "kind" in dataType) {
    return PIN_COLORS[(dataType as DataType).kind] ?? "var(--muted-foreground)";
  }
  return "var(--muted-foreground)";
}
