import { graphOutputKey } from "@/features/domain/editorProjection";
import type { PortAddressDto } from "@/shared/types/domain/editorProjection";

export function pinPreviewCacheKey(graphPath: string, port: PortAddressDto): string {
  return graphOutputKey({ graphPath, port });
}

export function lookupPinPreview<T>(
  previews: ReadonlyMap<string, T> | undefined,
  graphPath: string,
  port: PortAddressDto,
): T | undefined {
  return previews?.get(pinPreviewCacheKey(graphPath, port));
}
