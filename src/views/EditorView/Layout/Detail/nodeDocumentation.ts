import type { NodeDocumentation } from '@/shared/types/domain/node';

export function pickNodeDocumentation(
  doc: NodeDocumentation | undefined,
  language: string,
): string | undefined {
  if (!doc) return undefined;
  const isZh = language.startsWith('zh');
  const primary = isZh ? doc.zh : doc.en;
  const fallback = isZh ? doc.en : doc.zh;
  return primary ?? fallback;
}

export function resolveNodeDescription(
  documentation: NodeDocumentation | undefined,
  description: string | undefined,
  language: string,
): string | undefined {
  return pickNodeDocumentation(documentation, language) ?? description;
}
