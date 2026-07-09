import type { NodeMetaData } from '@/shared/types/domain/node';
import { pickLocalizedText } from '@/shared/types/domain/node';

export { pickLocalizedText };

export function resolveNodeDocumentationContent(
  meta: NodeMetaData | undefined,
  language: string,
  instanceDescription?: string,
): string | undefined {
  return pickLocalizedText(meta?.documentation, language) ?? instanceDescription;
}
