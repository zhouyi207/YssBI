import { graphOutputKey } from "@/features/domain/editorProjection";
import type { PortAddressDto } from "@/shared/types/domain/editorProjection";

function addressedPinCacheKey(graphPath: string, port: PortAddressDto): string {
  return graphOutputKey({ graphPath, port });
}

export function pinPreviewCacheKey(graphPath: string, port: PortAddressDto): string {
  return addressedPinCacheKey(graphPath, port);
}

export function pinResultCacheKey(graphPath: string, output: PortAddressDto): string {
  return addressedPinCacheKey(graphPath, output);
}

export function lookupPinPreview<T>(
  previews: ReadonlyMap<string, T> | undefined,
  graphPath: string,
  port: PortAddressDto,
): T | undefined {
  return previews?.get(pinPreviewCacheKey(graphPath, port));
}
