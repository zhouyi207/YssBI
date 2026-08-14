import { portAddressKey } from '@/features/domain/editorProjection';
import type { PortAddressDto } from '@/shared/types/dto/editorProjection';
import type { PinHistoryProjection } from '@/shared/types/ui';

function addressedPinCacheKey(graphPath: string, port: PortAddressDto): string {
  return `${graphPath.length}:${graphPath}:${portAddressKey(port)}`;
}

export function pinPreviewCacheKey(graphPath: string, port: PortAddressDto): string {
  return addressedPinCacheKey(graphPath, port);
}

export function pinHistoryCacheKey(graphPath: string, output: PortAddressDto): string {
  return addressedPinCacheKey(graphPath, output);
}

export function lookupPinPreview<T>(
  previews: ReadonlyMap<string, T> | undefined,
  graphPath: string,
  port: PortAddressDto,
): T | undefined {
  return previews?.get(pinPreviewCacheKey(graphPath, port));
}

export function lookupPinHistory(
  histories: ReadonlyMap<string, PinHistoryProjection> | undefined,
  graphPath: string,
  output: PortAddressDto,
): PinHistoryProjection | undefined {
  return histories?.get(pinHistoryCacheKey(graphPath, output));
}
