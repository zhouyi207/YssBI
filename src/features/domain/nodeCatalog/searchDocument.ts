import { pinyin } from 'pinyin-pro';
import type { LocalizedCatalogItemDto } from '@/shared/types/domain/localizedCatalog';

export interface CatalogSearchDocument {
  nodeTypeId: string;
  localizedTitle: string;
  aliases: string[];
  technicalTerms: string[];
  backendSearchText: string[];
  resourceNames: string[];
  pinyinFull: string[];
  pinyinInitials: string[];
}

const HAN_CHARACTER = /\p{Script=Han}/u;
const PINYIN_OPTIONS = {
  toneType: 'none',
  type: 'array',
  mode: 'normal',
  nonZh: 'consecutive',
} as const;

export function normalizeCatalogSearchText(value: string): string {
  return value
    .normalize('NFKD')
    .replace(/\p{Mark}/gu, '')
    .toLowerCase()
    .replace(/[^\p{Letter}\p{Number}]+/gu, ' ')
    .trim();
}

function normalizedUnique(values: Iterable<string>): string[] {
  return [...new Set(
    [...values]
      .map(normalizeCatalogSearchText)
      .filter(Boolean),
  )];
}

function pinyinForms(values: readonly string[], pattern: 'pinyin' | 'first'): string[] {
  return normalizedUnique(values
    .filter((value) => HAN_CHARACTER.test(value))
    .map((value) => pinyin(value, {
      ...PINYIN_OPTIONS,
      pattern,
    }).join(pattern === 'first' ? '' : ' ')));
}

export function buildCatalogSearchDocument(
  item: LocalizedCatalogItemDto,
): CatalogSearchDocument {
  const localizedTitle = normalizeCatalogSearchText(item.title);
  const aliases = normalizedUnique(item.aliases);
  const technicalTerms = normalizedUnique(item.technicalTerms);
  const backendSearchText = normalizedUnique(item.backendSearchText);
  const resourceNames = normalizedUnique(item.resourceNames);
  const pinyinSources = [
    item.title,
    ...item.aliases,
    ...item.technicalTerms,
    ...item.backendSearchText,
    ...item.resourceNames,
  ];

  return {
    nodeTypeId: item.nodeTypeId,
    localizedTitle,
    aliases,
    technicalTerms,
    backendSearchText,
    resourceNames,
    pinyinFull: pinyinForms(pinyinSources, 'pinyin'),
    pinyinInitials: pinyinForms(pinyinSources, 'first'),
  };
}

export function matchesCatalogSearchDocument(
  document: CatalogSearchDocument,
  query: string,
): boolean {
  const terms = normalizeCatalogSearchText(query).split(' ').filter(Boolean);
  if (terms.length === 0) return true;

  const text = normalizedUnique([
    document.nodeTypeId,
    document.localizedTitle,
    ...document.aliases,
    ...document.technicalTerms,
    ...document.backendSearchText,
    ...document.resourceNames,
    ...document.pinyinFull,
    ...document.pinyinInitials,
  ]).join(' ');
  return terms.every((term) => text.includes(term));
}
