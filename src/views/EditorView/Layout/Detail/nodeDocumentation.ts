import type { NodeMetaData } from '@/shared/types/domain/node';
import { pickLocalizedText } from '@/shared/types/domain/node';

export { pickLocalizedText };

/** @deprecated 使用 pickLocalizedText */
export const pickNodeDocumentation = pickLocalizedText;

export function resolveNodeDocumentationContent(
  meta: NodeMetaData | undefined,
  language: string,
  instanceDescription?: string,
): string | undefined {
  return pickLocalizedText(meta?.documentation, language) ?? instanceDescription;
}

/** @deprecated 使用 resolveNodeDocumentationContent */
export function resolveNodeDescription(
  documentation: NodeMetaData['documentation'],
  description: string | undefined,
  language: string,
): string | undefined {
  return pickLocalizedText(documentation, language) ?? description;
}
